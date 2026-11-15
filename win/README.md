# Windows build (x86_64)

ELABS RLX was written for macOS. This directory holds everything needed to build the same plugin as a
native Windows VST3, without Visual Studio: Rust's `x86_64-pc-windows-gnu` target plus **Zig** as the
C compiler, archiver and linker.

```
cargo xtask bundle elabs_lxr --release        ->  target/bundled/ELABS RLX.vst3
```

## What had to change

| File | Change |
|---|---|
| `build.rs` | Builds `lxr_engine.dll` instead of `lxr_engine.dylib` on Windows (the macOS branch is untouched), and takes the C compiler from `CC` / `CC_x86_64_pc_windows_gnu`. |
| `src/engine.rs` | Embeds `lxr_engine.dll` on Windows and writes the per-instance copy with a `.dll` extension (`libloading`/`LoadLibrary` then gives every instance its own copy of the firmware's globals, exactly as the `.dylib` did). |
| `engine/shim/stm32f7xx_hal.h` | `arc4random()` (BSD/macOS) is replaced by a small xorshift under `_WIN32`. It only seeds noise. |
| `.cargo/config.toml` | Points the linker and the C compiler at `win/zigcc.exe` and sets `ZIG`. Contains absolute paths — adjust them if the checkout moves. |
| `win/zigtool.c` | Builds into `zigcc.exe`, `dlltool.exe` and `ar.exe`: thin front ends for `zig cc` / `zig dlltool` / `zig ar` that also translate what rustc and the `cc` crate expect (`--target=x86_64-pc-windows-gnu` → Zig's triple, Rust's `-nodefaultlibs`/`-lmsvcrt` are dropped, `-L`/`-l` names resolved to MinGW `.a` files, `-Wl,....def` replaced by `--export-all-symbols`, `-lsynchronization` pointed at `win/syslib/libsynchronization.a`). |
| `win/syslib/` | `synchronization.def` + the generated import library: `WaitOnAddress`, `WakeByAddressSingle`, `WakeByAddressAll`. Rust's std uses them; Zig ships no import library for that API set. |
| `Cargo.toml` | Adds `windows-sys` for the keyboard hooks, and two `[patch]` entries that point egui-baseview and baseview at the vendored copies under `vendor/`. |
| `vendor/egui-baseview` | Same crate as upstream, with one change: the keyboard focus is taken on **every mouse press** (`window.focus()`), not only when egui reports an IME area. Bitwig does not hand a parented editor window the keyboard otherwise, and the old IME heuristic worked only now and then. |
| `vendor/baseview` | Same crate as upstream, with one change in `src/win/keyboard.rs`: a control character (`WM_CHAR` below `0x20`) carries its own key (`VK_BACK`, `VK_RETURN`, `VK_TAB`, `VK_ESCAPE`) when no key down preceded it. Otherwise such a character became `Key::Unidentified` — Backspace from the low-level hook did nothing in Ableton. |
| `src/editor/keyhook.rs` | Two hooks, both installed on the editor thread. (1) `WH_GETMESSAGE`: Windows' `TranslateMessage` produces no `WM_CHAR` for Shift combinations and punctuation while the editor is a child window in a DAW (Bitwig: `WM_KEYDOWN` for VK_SHIFT and VK_A, no `WM_CHAR` at all); the hook asks the keyboard layout for the character and posts one to the editor if none arrives by the key release. (2) `WH_KEYBOARD_LL`: REAPER and Ableton never give the editor the keyboard, so while a text field is focused and the pointer is over the panel the keys are taken from the system and handed **straight to egui** (`take_typed`, drained at the start of every frame) — posting a `WM_CHAR` to the window does not work there, because baseview only turns a character into a key event when a matching `WM_KEYDOWN` went through the same window. |
| `standalone.cmd` | Starts the standalone preview with `-p 2048`: nih-plug's WASAPI backend asserts when Windows delivers more samples than the configured period (this machine: 1056), the audio thread dies, and with it every parameter change from the GUI. |

## Toolchain

1. **Zig** (only needs unpacking): https://ziglang.org/download → e.g. `.toolchain/zig-x86_64-windows-0.17.1/`.
2. **Rust** with the GNU toolchain:
   ```
   rustup toolchain install stable-x86_64-pc-windows-gnu
   rustup default stable-x86_64-pc-windows-gnu
   ```
3. The three wrappers, once (and after editing `zigtool.c`):
   ```
   zig cc -target x86_64-windows-gnu -O2 -o win/zigcc.exe   win/zigtool.c
   zig cc -target x86_64-windows-gnu -O2 -o win/dlltool.exe win/zigtool.c
   zig cc -target x86_64-windows-gnu -O2 -o win/ar.exe      win/zigtool.c
   ```
   `win/` must be on `PATH` for the build (`dlltool` and `ar` are looked up there).

## Install

Copy `target/bundled/ELABS RLX.vst3` to
`%LOCALAPPDATA%\Programs\Common\VST3\` (no admin rights) or `C:\Program Files\Common Files\VST3\`
(both are standard VST3 locations). Then rescan the plug-ins in the DAW.

## Standalone preview

```
cargo run --release --features preview --bin lxr_preview
```

opens the panel as a normal Windows application — handy to check the GUI and the sound without a DAW.

**On Windows, always give it a large enough period:** `lxr_preview.exe -p 2048` (or use
`standalone.cmd`). nih-plug's WASAPI backend asserts when Windows hands it more samples than the
configured period — this machine's shared-mode WASAPI delivers 1056 regardless of a 512 or 1024
request — and the audio thread panics. nih-plug applies GUI parameter changes *on the audio thread*,
so with that thread gone the whole panel looks dead. `-b dummy` starts it without audio.
