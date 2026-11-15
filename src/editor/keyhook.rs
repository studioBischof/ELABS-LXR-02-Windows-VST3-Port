//! A message hook on the editor's own UI thread.
//!
//! baseview reads text from `WM_CHAR`, and Windows' `TranslateMessage` refuses to produce one for
//! Shift combinations and punctuation while the editor is a child window inside a DAW (seen with
//! Bitwig: the log shows WM_KEYDOWN for VK_SHIFT and VK_A but no WM_CHAR at all). The key presses
//! still arrive, so this hook translates them itself: on key down it asks the keyboard layout for
//! the character, and if no `WM_CHAR` follows by the time the key is released, it posts one to the
//! editor window. Control keys (Backspace, Enter, arrows) are left alone - egui handles those
//! through `Event::Key`.
#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardState, ToUnicodeEx,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, HHOOK, MSG, WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN,
    WM_KEYUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static INSTALLED: AtomicBool = AtomicBool::new(false);
static mut HOOK: HHOOK = std::ptr::null_mut();
/// the character the last key down would produce, waiting to see whether Windows sends it itself
static PENDING: AtomicU32 = AtomicU32::new(0);

/// The printable character this key press stands for, if any.
unsafe fn char_for(msg: &MSG) -> Option<u32> {
    let vk = msg.wParam as u32;
    let scan = ((msg.lParam >> 16) & 0xff) as u32;
    let mut state = [0u8; 256];
    if GetKeyboardState(state.as_mut_ptr()) == 0 {
        return None;
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

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = &*(lparam as *const MSG);
        match msg.message {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                PENDING.store(char_for(msg).unwrap_or(0), Ordering::Release);
            }
            WM_KEYUP | WM_SYSKEYUP => {
                // no WM_CHAR arrived for this key: make the character ourselves
                let c = PENDING.swap(0, Ordering::AcqRel);
                if c != 0 {
                    PostMessageW(msg.hwnd, WM_CHAR, c as WPARAM, msg.lParam);
                }
            }
            WM_CHAR | WM_SYSCHAR => {
                // Windows produced it after all: keep that one, do not post a second
                PENDING.store(0, Ordering::Release);
            }
            _ => {}
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// Installs the hook once, on the thread that runs the editor's message loop.
pub fn install() {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }
    unsafe {
        let thread = GetCurrentThreadId();
        HOOK = SetWindowsHookExW(WH_GETMESSAGE, Some(hook_proc), std::ptr::null_mut(), thread);
    }
}
