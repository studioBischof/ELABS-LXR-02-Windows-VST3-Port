//! Keyboard handling for the editor window.
//!
//! Two mechanisms, both Windows-only:
//!
//! * **Character translation** (`WH_GETMESSAGE` on the editor thread). Windows' `TranslateMessage`
//!   refuses to produce a `WM_CHAR` for Shift combinations and punctuation while the editor is a
//!   child window inside a DAW (Bitwig: `WM_KEYDOWN` for VK_SHIFT and VK_A, no `WM_CHAR` at all).
//!   The hook asks the keyboard layout for the character and posts one if none arrives by the key
//!   release. Backspace, Enter and the arrows are left alone - egui reads those as `Event::Key`.
//! * **Typing where the host keeps the keyboard** (`WH_KEYBOARD_LL`). REAPER and Ableton give the
//!   embedded editor no keyboard focus: the keys go to their own window. While a text field in the
//!   panel has the focus, the pointer is over the panel and the editor has no focus of its own, the
//!   low-level hook takes the character and hands it to egui directly ([`take_typed`], drained at
//!   the start of every frame). Posting a `WM_CHAR` to the editor does not work there: baseview
//!   only turns a character into a key event when the matching `WM_KEYDOWN` went through the same
//!   window (REAPER: the letters reached the window and were dropped again). The key itself is
//!   swallowed while the pointer is over the panel, so nothing leaks into the host.
#![cfg(windows)]

use std::cell::{Cell, RefCell};

use nih_plug_egui::egui;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_BACK, VK_CONTROL, VK_MENU,
    VK_RETURN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetClassNameW, GetCursorPos, GetForegroundWindow, GetParent,
    KBDLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW, WindowFromPoint, HC_ACTION, MSG,
    WH_GETMESSAGE, WH_KEYBOARD_LL, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

thread_local! {
    /// one hook set per editor thread: every instance has its own window and message loop
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
    /// a text field of this editor has the keyboard focus
    static WANT_KEYS: Cell<bool> = const { Cell::new(false) };
    /// this editor's own window, learned from its messages
    static EDITOR: Cell<isize> = const { Cell::new(0) };
    /// the character the last key down would produce, waiting to see whether Windows sends it
    static PENDING: Cell<u32> = const { Cell::new(0) };
    /// the virtual key we took from the system, so only its own release is swallowed as well
    static CAPTURED: Cell<u32> = const { Cell::new(0) };
    /// text typed while the host kept the keyboard: handed to egui at the start of the next frame
    static TYPED: RefCell<Vec<egui::Event>> = const { RefCell::new(Vec::new()) };
}

/// The text typed while the host owned the keyboard, as egui events.
pub fn take_typed() -> Vec<egui::Event> {
    TYPED.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

fn type_char(c: char) {
    TYPED.with(|q| q.borrow_mut().push(egui::Event::Text(c.to_string())));
}

fn type_key(key: egui::Key) {
    TYPED.with(|q| {
        q.borrow_mut().push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        })
    });
}

fn is_editor_class(hwnd: HWND) -> bool {
    let mut class = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
    if n <= 0 {
        return false;
    }
    String::from_utf16_lossy(&class[..n as usize]).starts_with("Baseview-")
}

/// The editor window under the pointer, if the pointer is over one.
unsafe fn editor_under_pointer() -> Option<HWND> {
    let mut p = POINT { x: 0, y: 0 };
    if GetCursorPos(&mut p) == 0 {
        return None;
    }
    let mut hwnd = WindowFromPoint(p);
    for _ in 0..8 {
        if hwnd.is_null() {
            return None;
        }
        if is_editor_class(hwnd) {
            return Some(hwnd);
        }
        hwnd = GetParent(hwnd);
    }
    None
}

/// The printable character this key press stands for, if any.
unsafe fn char_for(vk: u32, scan: u32) -> Option<u32> {
    let mut state = [0u8; 256];
    if GetKeyboardState(state.as_mut_ptr()) == 0 {
        return None;
    }
    // a thread that is not the foreground thread gets an empty key state (REAPER hosts the editor
    // on its main thread but translates keys elsewhere): build the modifiers by hand
    if state.iter().all(|&b| b == 0) {
        if GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000 != 0 {
            state[VK_SHIFT as usize] = 0x80;
        }
        if GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000 != 0 {
            state[VK_CONTROL as usize] = 0x80;
        }
        if GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000 != 0 {
            state[VK_MENU as usize] = 0x80;
        }
    }
    let mut buf = [0u16; 8];
    let layout = GetKeyboardLayout(GetCurrentThreadId());
    // flag 4: do not change the keyboard state (a dead key must stay armed)
    let n = ToUnicodeEx(vk, scan, state.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, 4, layout);
    if n == 1 && buf[0] as u32 >= 0x20 {
        Some(buf[0] as u32)
    } else {
        None
    }
}

unsafe extern "system" fn message_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = &*(lparam as *const MSG);
        if EDITOR.with(|e| e.get()) == 0 && is_editor_class(msg.hwnd) {
            EDITOR.with(|e| e.set(msg.hwnd as isize));
        }
        match msg.message {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let scan = ((msg.lParam >> 16) & 0xff) as u32;
                PENDING.with(|p| p.set(char_for(msg.wParam as u32, scan).unwrap_or(0)));
            }
            WM_KEYUP | WM_SYSKEYUP => {
                // no WM_CHAR arrived for this key: make the character ourselves
                let c = PENDING.with(|p| p.replace(0));
                if c != 0 {
                    PostMessageW(msg.hwnd, WM_CHAR, c as WPARAM, msg.lParam);
                }
            }
            WM_CHAR | WM_SYSCHAR => {
                // Windows produced it after all: keep that one, do not post a second
                PENDING.with(|p| p.set(0));
            }
            _ => {}
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// While a text field has the focus, type into the editor even if the host holds the keyboard.
unsafe extern "system" fn low_level_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code as u32 == HC_ACTION && (wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN) {
        // only when a text field is focused and the host kept the keyboard
        let want = WANT_KEYS.with(|w| w.get());
        // is the window with the keyboard one of ours? if not, the host kept it
        let foreground = GetForegroundWindow();
        let host_has_keyboard = foreground.is_null() || !is_editor_class(foreground);
        let under = editor_under_pointer().is_some();
        if want && host_has_keyboard && under {
            let kbd = &*(lparam as *const KBDLLHOOKSTRUCT);
            // hand the text to egui directly: a WM_CHAR posted to the editor window is not
            // translated by baseview inside REAPER, and egui never sees those keys otherwise
            let mut typed = false;
            if let Some(c) = char_for(kbd.vkCode, kbd.scanCode) {
                if let Some(ch) = char::from_u32(c) {
                    type_char(ch);
                    typed = true;
                }
            }
            if !typed {
                if kbd.vkCode == VK_BACK as u32 {
                    type_key(egui::Key::Backspace);
                    typed = true;
                } else if kbd.vkCode == VK_RETURN as u32 {
                    type_key(egui::Key::Enter);
                    typed = true;
                }
            }
            if typed {
                CAPTURED.with(|c| c.set(kbd.vkCode));
                return 1;
            }
        }
    }
    if code as u32 == HC_ACTION && (wparam as u32 == WM_KEYUP || wparam as u32 == WM_SYSKEYUP) {
        // only the release of a key we took, so nothing typed elsewhere is ever swallowed
        let kbd = &*(lparam as *const KBDLLHOOKSTRUCT);
        if CAPTURED.with(|c| c.replace(0)) == kbd.vkCode {
            return 1;
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// Installs the hooks once per editor thread.
pub fn install() {
    if INSTALLED.with(|i| i.replace(true)) {
        return;
    }
    unsafe {
        let thread = GetCurrentThreadId();
        let _ = SetWindowsHookExW(WH_GETMESSAGE, Some(message_hook), std::ptr::null_mut(), thread);
        // a low level hook needs the module that contains the procedure
        let mut module = std::ptr::null_mut();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            low_level_hook as *const u16,
            &mut module,
        );
        let _ = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_hook), module, 0);
    }
}

/// Told every frame: does a text field of this editor have the keyboard focus?
pub fn set_want_keys(on: bool) {
    WANT_KEYS.with(|w| w.set(on));
}

