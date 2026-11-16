# A Windows x64 VST3 Port of the ELABS LXR-02 Clone

This project ports the macOS ELABS-RLX-02-LXR02-CLONE-VST (Rust + nih-plug + egui) to run on Windows 11 as a VST3. Some minor features were also added.

## 1. Panel additions in this fork

* **Voice COPY / PASTE** (bottom row, next to the size presets): copies all 37 parameters of the
  selected voice plus its fader, survives loading another kit, and pastes between voices of the same
  type (Drum 1/2/3 — CL HH and OP HH are the same hihat voice anyway).
* **CONFIG page** (step key 12, and a CONFIG button in the bottom row): the kit's master sample rate -
  the sample-rate reduction kits like LOFIDUST store well below 127, the only parameter with no panel
  control before - plus **Master Tune** (−24…+24 semitones for the whole kit) and the **morph amount**,
  so the three master settings can be performed from one page.
* **MORPH page** (step key 9, "MORPH"): knob 1 picks kit A (and loads it while turning), knob 2 picks
  the morph target kit B, knob 4 (or DATA) is the morph amount. The display shows both kits with the
  amount right-aligned. The morph target and the amount belong to the project, not to the kit, so
  loading another kit A keeps B. Knob 3 saves what is heard (morph included) as a new kit: it opens
  the SAVE page for the name, then loads the new kit, puts the morph back to 0 (the baked values
  already contain it) and returns to the kit A / kit B display.
* **Delete Preset** in the right detail view of the browser — DELETE KIT on the KITS tab, DELETE
  PATTERN on the PATTERNS tab: the first click turns the button into "REALLY DELETE?", the second
  removes the file (kit `.SND` or pattern `.PAT`). Built-in banks and projects have no file, so they
  are left alone.
* **Lock Kit when loading a pattern** — on the PATTERNS tab a switch at the left, above the project
  list: while it is on, clicking a pattern loads only its steps and the sound stays as it is. It is the
  same switch as knob 3 on the PATTERN page ("kit follows: off"), so both stay in step.
* **Note Lock** — the switch under it, on the KITS *and* the PATTERNS tab (on by default): while it is
  on, every step plays at the pitch set in the kit and the note stored in the pattern is ignored. The
  factory SD content carries note 63 in every step (+3 semitones; 60 is the LXR's reference — the kit's
  own pitch), patterns made here use 60, so switching between both used to move every voice by three
  semitones. The pattern data itself is not changed: the STEP page still shows the note that is stored.
* **Master Tune** (CONFIG page, knob 2): tunes the whole kit up or down in semitones (−24…+24). It moves
  what is played — the sequencer's steps, the panel keys and MIDI notes — and belongs to the project,
  not to the kit, so loading another kit keeps it.
---

## 2. Download the factory LXR presets

They can be imported as .ZIP inside the VST3 under Browser.

Original LXR Presets:
http://sonic-potions.com/public/SdCardImage.zip

LXR-01 Presets under Downloads:
https://www.ericasynths.lv/drum-synthesizer-lxr-02-1132/

---

## 3. Files changed or added

| File | Change |
|---|---|
| `build.rs` | Builds the firmware as **`lxr_engine.dll`** instead of `.dylib` (the macOS branch is untouched); C compiler taken from `CC`/`CC_x86_64_pc_windows_gnu`. |
| `src/engine.rs` | Embeds `lxr_engine.dll` and writes a `.dll` copy per instance. **New resampler** from 44.140 kHz to the host rate (64-tap Blackman windowed sinc, 512 phases) instead of cubic interpolation. Plus a test that a voice set to St2 plays on output 2 and stays off output 1. |
| `src/library.rs` | Library folder from `HOME` **or** `USERPROFILE` (Windows has no `HOME`); `ensure_root()` creates `Kits/Projects/Imported`. |
| `src/lib.rs` | Calls `ensure_root()` during initialisation. The second output is decided per block from the buffer the host actually passes (a flag taken at initialisation stayed false when the host created the aux bus after loading), and the voice clipboard lives here. |
| `src/editor.rs` | `zoom()` now uses the **actual** window size (`ctx.screen_rect()`) instead of the requested one - otherwise the standalone crops the panel and clicks land in the wrong place. Also the CONFIG and MORPH pages and the voice COPY/PASTE buttons (section 1). |
| `src/ui_pages.rs` | The CONFIG page (master sample rate and master tune) and the MORPH page's slots. |
| `src/editor/history.rs` | `say()` for the panel's message line, and `commit` public so a paste is its own undo step. |
| `src/editor/browser.rs` | Stable widget ids; **own list scrolling** instead of `ScrollArea` (offset kept in `BrowserState`, own scrollbar); name field no longer swallowed by the 8-character limit; "SHOW THE LIBRARY FOLDER" uses `explorer`; import/save/tag areas got their own ids; **DELETE KIT / DELETE PATTERN** for the selected item, asking once more before the file is removed. |
| `src/editor/save.rs` | Name field keeps the **first** 8 characters (the .SND name is 8 bytes) instead of the last 8 - a name with a space looked like only the part behind the space had been stored. |
| `src/library.rs` | `delete_pattern()` removes a pattern's `.PAT` file from its project folder (the kit file next to it stays); projects carry the folder they were read from. |
| `engine/shim/stm32f7xx_hal.h` | `arc4random()` (BSD/macOS) → xorshift under `_WIN32` (noise seeding only). |
| `engine/lxr_entropia.c` | Comment only: ENTROPIA MOD sums both outputs into Out 1 on purpose, so per-voice outputs do not apply while it is on. |
| `Cargo.toml` | Extra direct dependency on `egui-baseview` with the **`windows_keyboard_workaround`** feature → keyboard input in text fields, plus `windows-sys` for the keyboard hooks and two `[patch]` entries pointing `egui-baseview` and `baseview` at `vendor/`. |
| `vendor/egui-baseview` | Upstream crate with one change: the keyboard focus is taken on **every mouse press** (Bitwig hands a parented editor window no keyboard otherwise). |
| `vendor/baseview` | Upstream crate with one change in `src/win/keyboard.rs`: a control character (`WM_CHAR` below `0x20`) carries its own key (`VK_BACK`, `VK_RETURN`, `VK_TAB`, `VK_ESCAPE`) when no key down preceded it — otherwise Backspace from the low-level hook was `Key::Unidentified`. |
| `src/editor/keyhook.rs` | Keyboard for hosts that do not pass keys to the editor: a `WH_GETMESSAGE` hook that asks the layout for missing characters (Bitwig), and a `WH_KEYBOARD_LL` hook that hands characters straight to egui while a text field is focused and the pointer is over the panel (REAPER, Ableton). |
| `.cargo/config.toml` | Target/linker/env for the Windows build. |
| `win/*` | Toolchain wrappers, import library, `README.md` (build documentation). |
| `install-vst3.ps1` | Copies the bundle to `C:\Program Files\Common Files\VST3` (asks for admin). |
| `standalone.cmd` | Starts the standalone with `-p 2048`. |

--- 

## 4. Toolchain (no Visual Studio, no CMake)

* **Rust** with the GNU target:
  ```
  rustup toolchain install stable-x86_64-pc-windows-gnu
  rustup default stable-x86_64-pc-windows-gnu
  ```
* **Zig 0.17.1** (just unpacked, no installer): used as the C compiler, archiver and linker.
* **`win/zigtool.c`**, built three times as `zigcc.exe`, `dlltool.exe` and `ar.exe`. The program picks
  `zig cc` / `zig dlltool` / `zig ar` from its own file name and translates what Rust expects
  (details in section 9).
* **`win/syslib/libsynchronization.a`** – a hand-made import library for `WaitOnAddress`,
  `WakeByAddressSingle` and `WakeByAddressAll` (Zig ships none).
* **`.cargo/config.toml`** sets the linker, `CC_x86_64_pc_windows_gnu` and `ZIG` (absolute paths!).

If the folder moves, the three paths in `.cargo/config.toml` must be updated.

---

## 5. Build and install

```powershell
cd C:\Users\bjoern\Documents\LXR-02\ELABS-RLX-02-LXR02-CLONE-VST-main
$env:Path = "$PWD\win;$env:USERPROFILE\.cargo\bin;$env:Path"

cargo test --release                      # 15 tests
cargo xtask bundle elabs_lxr --release    # -> target\bundled\ELABS RLX.vst3
powershell -ExecutionPolicy Bypass -File .\install-vst3.ps1   # UAC, installs into Program Files
```

`win/` must be on `PATH` — otherwise the `cc` crate cannot find `dlltool`/`ar`.

**Important:** close the DAW before copying (the DLL is locked otherwise) and restart it afterwards.

---

## 6. Picking this up in three steps

1. Read `win/README.md` in the project (build details) — or this text from section 3 on.
2. Run `cargo test --release`; if the linker is missing, check `.cargo/config.toml`
   (`ZIG` path, `zigcc.exe`) and rebuild the wrappers if needed:
   ```
   zig cc -target x86_64-windows-gnu -O2 -o win/zigcc.exe   win/zigtool.c
   zig cc -target x86_64-windows-gnu -O2 -o win/dlltool.exe win/zigtool.c
   zig cc -target x86_64-windows-gnu -O2 -o win/ar.exe      win/zigtool.c
   ```
3. `cargo xtask bundle elabs_lxr --release` + `install-vst3.ps1`.

---

## 7. Debug history (so none of it is searched twice)

| Symptom | Cause / fix |
|---|---|
| `error: unable to find dynamic system library "msvcrt"` | Rust passes `-nodefaultlibs`; Zig then drops its own libc paths → removed in the wrapper. Same for `-lmsvcrt`, `-l:libpthread.a`. |
| `undefined symbol: WaitOnAddress` | Zig has no import library for it → `win/syslib/libsynchronization.a` (imported from `api-ms-win-core-synch-l1-2-0.dll`; this system's `kernel32.dll` does not export those symbols). |
| `unsupported linker arg: …list.def` | rustc hands proc-macro export lists over as a `.def` → replaced with `-Wl,--export-all-symbols`. |
| arguments inside `@response-file` not filtered | rustc puts long link lines into a response file (one argument per line, backslashes doubled) → the wrapper reads and rewrites it. |
| `unable to find dynamic system library "windows"` | `-l<name>` makes Zig look for a DLL; the wrapper resolves `-l`/`-L` to existing `.a` files. |
| `unable to parse target query 'x86_64-pc-windows-gnu'` | Zig wants `x86_64-windows-gnu` → mapped in the wrapper. |
| `call to undeclared function 'arc4random'` | macOS API in the firmware shim → xorshift under `_WIN32`. |
| `cc-rs: failed to find tool "ar"` / `dlltool` | provide wrappers as `ar.exe`/`dlltool.exe` and put `win/` on `PATH`. |
| standalone panel does not react | WASAPI panic (1056 > 512) → `-p 2048`. |
| text fields take no keystrokes | baseview never calls `SetFocus` for a parented window on Windows → egui-baseview's `windows_keyboard_workaround`, and a `window.focus()` on every mouse press (Bitwig). |
| no `WM_CHAR` for Shift and punctuation in Bitwig | `TranslateMessage` does not translate them for a child window inside a DAW → `WH_GETMESSAGE` hook translates the key itself and posts the character. |
| no typing at all in REAPER and Ableton | the hosts keep the keyboard on their own window and the editor is never focused → `WH_KEYBOARD_LL` hook takes the keys while a text field is focused and the pointer is over the panel. |
| letters reached the editor window in REAPER but never appeared | a posted `WM_CHAR` does not survive baseview: it only builds a `Key` event when the matching `WM_KEYDOWN` went through the same window → hand the text to egui directly (`keyhook::take_typed`, drained at the start of the frame) instead of posting messages. |
| Backspace did nothing in Ableton | a `WM_CHAR` `0x08` without a key down became `Key::Unidentified` → patched `vendor/baseview` (control character carries its key). |
| Enter stopped working everywhere while the plugin was open | the low-level hook swallowed the release of *every* key while a text field had the focus → it now only swallows the release of a key it actually took. |
| list jumps when a kit is picked | unstable egui ids plus `ScrollArea` bookkeeping → stable ids, then own scrolling. |
| panel cropped in the standalone, clicks miss | `zoom` used the requested size instead of the real window size. |
| "no home folder" when importing | Windows has no `HOME` → `USERPROFILE`. |
| scrollbar invisible | drawn with the painter clipped to the row area → use the unclipped painter. |



---

# Original Description for ELABS RLX for macOS

**The LXR-02 drum engine as a free VST3 for macOS (Apple Silicon and Intel).**
It isn't an emulation: it's the open-source LXR V2 firmware's own C sound engine and sequencer, compiled for the Mac and
wrapped in a panel laid out like the hardware. Made by EntropiaLabs.

![ELABS RLX panel](docs/panel.jpg)

- six synthesized drum voices (three drums, snare, clap/cymbal, hi-hats) with FM, transient clicks,
  filters, LFOs, velocity modulation and kit morphing
- the hardware's step sequencer synced to the DAW: 64 patterns of 7 tracks, parameter locks,
  probability, flams, euclidean patterns, shuffle, song mode and a LANES view that edits every step
- **ENTROPIA MOD**: per-voice sends to delay, ring modulator and reverb, then a master filter,
  drive and compressor
- **the Maestro**: plays your pattern like a drummer, with variations, fills and moves on decays,
  sends, LFO and filter
- kits and patterns in the LXR-02's own .SND / .PAT formats; Erica Synths' free factory content
  imports straight from their zips (BROWSER > IMPORT)
- 11 kits and 7 patterns by EntropiaLabs built in, a preset browser with stars and tags, undo/redo,
  a resizable panel and any number of instances

**Download:** [ELABS RLX on Ko-fi](https://ko-fi.com/s/9348fe308a) (free, pay what you want) or the
[Releases](../../releases) page here. macOS 11 or newer, any VST3
host (built and tested in Ableton Live 12). The installer isn't signed with a paid Apple
certificate, so macOS asks once: right-click the .pkg > Open.

| | |
|---|---|
| ![Maestro](docs/maestro.jpg) | ![ENTROPIA MOD](docs/fx.jpg) |
| ![Preset browser](docs/browser.jpg) | |

## MIDI

| Channel | Notes | Plays |
|---|---|---|
| 1 | 36-42 (C1-F#1) | Drum 1, Drum 2, Drum 3, Snare, Cymbal, Hihat closed, Hihat open |
| 2-8 | any | the same 7 triggers chromatically, note 60 = programmed pitch |

## Panel

Laid out like the LXR-02 front panel (without the manufacturer logos):

- the LCD shows four parameters of the selected voice and sub page, exactly as the firmware's
  menu pages define them; the selected one is in capitals and its full name is shown below
- the four knobs edit those four; DATA edits the selected one in single steps
  (drag, scroll wheel, double-click resets)
- DRUM1 ... OP HH next to the faders select the voice, OSC ... MIX the sub page; press a sub page
  again (or use the BAR arrows) for its second row of parameters
- pads 1-7 play the voices and light up when a voice is triggered (also from MIDI); FX opens the
  master FX page, which follows the FX type
- faders: voice levels and master volume

`cargo run --release --features preview --bin lxr_preview` opens the panel as a standalone app.

## Kits

Six ELABS kits are built in (bank **ELABS**): TEKKNO, BOOM808, ELECTRO, LOFIDUST, INDUSTRL and
MINIMAL, each the firmware's init kit with its own settings, with one pattern each in the ELABS
project. They are written by `src/elabs_kits.rs` (`cargo test --release --lib write_elabs_content
-- --ignored --nocapture` tunes the drums by measuring them, writes `kits/elabs/ELABS` and renders
`target/elabs_kits_demo_48k.wav`). Five EntropiaLabs kits made on the panel follow in the same bank
(NTRPBT1, CHAPA, CONCRETO, FERRUGEM, MADRUGA), with the ENTROPIA project's pattern, in
`kits/elabs/ENTROPIA`. `tools/gen_kits.py` and `tools/gen_projects.py` pack `kits/elabs` into
`kits/lxr_kits.bin` / `kits/lxr_projects.bin`. The built-in kits come tagged (`BUILT_IN_TAGS` in
`src/favorites.rs`) until the user changes their tags; a library kit with a built-in kit's sound is
listed once.

The LXR-02 factory content and Erica Synths' sound packs are not built in. They are free
downloads on the LXR-02 product page; the browser's **IMPORT** button takes them as they come
(zip or unzipped folder) and copies their kits and projects into the library under `Imported/`
(the factory card shows up as bank FACTORY with projects FACTORY 1-6, the packs as HRTL DLC and
UNTZKITS).

### Window, LANES, help

- **SIZE** (bottom right) or the corner handle resizes the window, 50-250 %, keeping the panel's
  proportions; the size is saved with the session. The panel is drawn at its base size on a
  scaled egui layer (egui-baseview ignores egui's zoom factor); `vendor/nih_plug_egui` makes the
  resize request public and fixes two resize bugs (see `vendor/README.md`)
- **LANES**: all seven tracks of the shown pattern, 64 steps; click, drag to paint, right-click or
  ⌘-click to edit a step; per lane mute and length; brightness = velocity; **DUPLICATE ×2** /
  **HALVE ÷2** double or halve the pattern (see COPY/CLR)
- LANES **detail strip**: click a lane (or its name) and edit that track's steps as bars, like a
  DAW's velocity lane: VELOCITY, PROB, NOTE (±2 octaves), RATCHET (the firmware's flam x2-x8),
  SHIFT (micro timing) and LOCK 1/2 (parameter locks; ◀ ▶ pick the parameter). Drag to draw,
  right-click resets a step, RESET clears the whole lane. Only steps that are on are drawn
  (`src/editor/lane_detail.rs`)
- **MAESTRO** (the little robot in the LANES header): plays the pattern a little differently
  every bar, after the ELABS Liveset Player's Maestro. VARIATION: velocities breathe (the kick
  only ±6, never moved), closed-hat ghosts and ratchets, an off-beat open hat, snare/clap ghost
  notes, percs moving a sixteenth. FILL EVERY 4/8/16 bars (or FILL NOW): a fill on the last
  beat of the phrase (snare roll with rising ratchets, clap stutter, tom run on DRUM2/3, hat
  build, kick drop), never the same one twice in a row. MOVES, each with its own switch, CHANCE
  (how many bars, or for the filter how many fills) and DEPTH (how far): DECAY (hat and snare
  decays), COLOUR (hat/snare/cymbal filters, snare noise, cymbal and perc decays; no pitch,
  nothing of the kick), and in ENTROPIA MOD SENDS (a voice thrown into the delay or reverb for a
  bar, the snare or clap at fills), RING LFO (rate or note length, depth, now and then the wave)
  and FILTER (a master filter sweep over the fill, high pass rising or low pass closing, back on
  the downbeat; other bars only now and then a short one). Turn a setting and that's the new
  centre. TRACKS picks what it may touch. It plays a varied copy in the engine: your
  pattern is never changed, and it pauses (and gives everything back) while REC is on. The
  variation goes in on each bar's first beat, a fill on its third (`src/maestro.rs`; settings
  saved with the session)
- **UNDO / REDO** (footer and LANES, also ⌘Z / ⇧⌘Z when the host passes the keys on): every
  click, drag or run of scrolling is one step (sound, kits, patterns, mutes, song; up to 100);
  only what the step changed is put back, so automation and REC recording are left alone
  (`src/editor/history.rs`)
- the ELABS logo (Entropia sticker, `assets/logo.png`, from the StageDeck/Gig Manager icon) sits
  where the LXR-02 has its mascot; the panel follows the unit's look: black scalloped DATA knob,
  light knob caps, round step caps in square bezels with the beats (1, 5, 9, 13) on white bezels, pink LEDs,
  second-function labels in white boxes
- hovering any control explains it; turning a knob or fader shows its name and value next to it
- the browser's **LOCK** buttons keep voices (and the FX) when a kit is loaded, also when a
  pattern brings its kit

### Browser

Clicking the display on a voice or FX page (or the BROWSER button) opens a browser with every kit
and every pattern: search, filter by bank/project, by tag or by favorites, sort by stars, and click
to load (a pattern brings its kit). Its **SAVE** button saves from there: on KITS the current sound
as a kit, on PATTERNS the shown pattern with the sound you hear (added to the project folder you
name, in its first free slot) or the whole session as a project. Star ratings (1-5) and tags (preset ones like Techno, 808, Lo-fi
plus your own) are kept in `~/Music/ELABS RLX/favorites.tsv`, so they hold across all sessions;
items are keyed by name and a hash of their contents.

### Library folder

`~/Music/ELABS RLX` works like the LXR-02's SD card, so unzipped LXR-02 downloads can be dropped in
as they are:

- every `.SND` kit in it shows up in the kit browser, one bank per folder (`Kits/` is bank USER)
- every folder with `.PAT` files is a project (64 patterns + the `NN-NAME.SND` kits they name) and
  shows up in the PATTERN browser after the built-in ones

**SAVE** saves in the hardware's formats: KIT writes the current sound to `Kits/NAME.SND` and into a
kit slot of the session's project (the shown pattern then uses that kit); PROJECT writes the
session's patterns and project kits to `Projects/NAME/` (`NN-.PAT`, `NN-NAME.SND`). Type the name
in the field under the display (it opens selected, Enter saves) or edit it on the display: knob 3
moves the cursor, knob 4 or DATA change the letter.

**IMPORT** (browser top bar) copies LXR-02 content into the library: choose a zip (as downloaded
from Erica Synths) or a folder. Only `.SND` and `.PAT` files are copied, to `Imported/NAME/`
(Erica's factory zip becomes FACTORY, Sound Pack 1 HRTL DLC, Sound Pack 2 UNTZKITS). Kits with the
same sound as one already listed and empty INIT kits are skipped; patterns without steps are left
out of the PATTERN browser. SHOW THE LIBRARY FOLDER opens `~/Music/ELABS RLX` in Finder.

### Morph

On the LOAD page, knob 3 switches between loading a kit and loading it as the morph kit B; knob 4
(or the `Morph` parameter) morphs 0-255 from the kit towards kit B with the firmware's
`interpolate()`, so it never fully reaches B, like the hardware. Audio outputs and modulation
destinations don't morph. Kit B is saved with the session.

User samples aren't part of this firmware (its sample oscillator code is compiled out), so there
are none here either.

## Sequencer

The firmware's own step sequencer (`sequencer.c`), clocked by the host: 96 ticks per beat from
Live's musical position, with the pattern's shuffle, re-aligned after loops and jumps (the steps
before the play position are skipped silently, so a pattern sits on the host's bar grid).

- **PLAY** (or the `Sequencer` parameter) turns it on; it plays while the host transport runs
- the 16 step buttons then edit the selected voice's track (CL HH / OP HH are separate tracks);
  the white frame is the playing step; **BAR** ◀ ▶ picks which of the 4 bars they show
- **SHIFT** latches the buttons' printed functions (pads, KIT, FX, PATTERN)
- **PERF** opens the euclid generator and shuffle: `len` `stp` `rot` (1-16, 1-length, 0-15) for the
  track picked with the voice buttons, regenerating its steps and length as the LXR's euclid page
  does (a Rust port of `EuklidGenerator.c`, checked against the firmware for all 2416 settings),
  and `shf`, the shown pattern's shuffle (0-127: every second step up to 2/3 of a step late)
- **right-click** (or ⌘-click) a step to edit it: velocity / note / probability, flam / shift, and
  two parameter locks (destination + value), the LXR's step edit views, switched with ◀ ▶
- **REC** records pads and MIDI notes into the playing pattern, quantised to the nearest step
- **SHIFT + a voice button** mutes that track ("MUTE" shows above it)
- **COPY/CLR**: clear the track or the pattern, copy the pattern to another slot, copy the track
  to another track, or double (`dbl pat`: every track twice as long, its steps repeated; tracks
  over 32 steps stay) or halve (`hlf pat`: the steps after the new end are kept) the pattern
- **SONG** (with SHIFT): the firmware's song mode, a list of up to 63 patterns played in order, then
  looping or stopping
- the MIX page's `len` is the track's length (1-64 steps) in the shown pattern
- **PATTERN** opens the browser: knob 1 project, knob 2 pattern (1-64), DATA scrolls patterns,
  click DATA or the display to play it (at the end of the playing pattern while running). Knob 3
  (click) switches "kit follows pattern": the pattern's kit is loaded when the sequencer reaches it
  (while the panel is open)

Projects: the built-in ELABS project and every project in the library (saved, imported or
dropped in), 64 patterns each, with the kit each pattern names. Patterns are saved in the Live
set. Step velocity, probability, note, flam, shift and per-step automation from imported patterns
play back.

## ALL PARAMS

The ALL PARAMS button shows every parameter of a voice (or FX/master) at once, in the order of the
menu pages. Both master views — CLASSIC and ENTROPIA MOD — end with the same MASTER row: SampleRt,
Tune, Morph and Volume, so the global settings are MIDI-mappable from there too. With Live's
**Configure** on, touching a control there adds it to the device panel, where Live can MIDI-map it.

## ENTROPIA MOD FX

The LXR has one FX bus that runs one effect at a time, chosen by the kit (CLASSIC). ENTROPIA MOD is
a modification of it: the FX page has an **FX MODE** switch, and **ENTROPIA MOD** opens a page of
its own (the FX button goes there while it is on) where

- every voice has a **send** to a **DELAY** (normal / ping-pong), a **RING** modulator and a
  **REVERB**, each with its settings and a RETURN level, all running at once;
- the ring's frequency has an **LFO** (sine / tri / saw / square / random, 0.05 to 20 Hz, depth up
  to ±3 octaves around FREQ; depth 0 is off); **SYNC** puts it on the host's tempo in note lengths
  (4 bars to 1/32, dotted and triplets) and locks it to the song position while the host plays;
- the whole mix then goes through a **MASTER FILTER** (centre off, left a low pass closing to 50
  Hz, right a high pass opening to 8 kHz, with resonance), **DRIVE** (tube / fold / rat) and
  **COMP**; DRIVE and COMP start with MIX at 0 (off).

The effects are the firmware's own (the reverb is in the firmware but switched off on the
hardware); only the filter and the ring LFO are new (`engine/lxr_entropia.c`). Everything is a
plugin parameter, automatable and saved with the session; kits are untouched, so they still load
on the hardware. In ENTROPIA MOD both outputs are mixed into Out 1 and the kit's own FX bus is
bypassed. Bars: drag or scroll, double-click (or right-click) for the default, so the filter goes back to off. ALL PARAMS' FX tab has the same
CLASSIC / ENTROPIA MOD switch and shows the settings of the mode that is on.

## Outputs

`Out 1` is the main stereo output and `Out 2` an extra stereo output, matching the LXR's two
stereo outs (each voice's `Output` parameter routes it). If the host only uses the main output,
Out 2 is mixed into it.

## Parameters

215 sound parameters (all voices, LFOs, filters, transients, master FX) with the LXR's value ranges
and names, plus a volume fader per voice. Defaults are the firmware's init kit.
`src/param_table.rs` and `src/ui_pages.rs` are generated from the firmware tables by `tools/gen_params.sh`.

## Not done yet

- roll

## Several instances

The firmware keeps its state in C globals, so the engine is built as its own dylib (`build.rs`),
embedded in the plugin, and every instance loads a private copy of it from
`$TMPDIR/ELABS RLX/engine-<pid>-<n>.dylib` (removed again when the instance closes). Each copy has
its own globals, so any number of instances play independently.

## Tests

- `cargo test --release --lib`: renders at 48 kHz, checks pitch against the native 44.14 kHz rate
  and that parameter changes reach the engine; writes `target/lxr_demo_48k.wav`
- `engine/test/param_sweep.sh`: every parameter through its whole range, every built-in kit and
  4 bars of every project pattern, under AddressSanitizer; writes nothing
- the engine test also renders `target/lxr_kits_demo_48k.wav`, a beat on four of the kits, and
  `target/lxr_sequencer_demo_48k.wav` (ELABS pattern 1 through the host-synced sequencer); it
  checks that every built-in pattern survives a round trip through the firmware byte for byte, that
  the euclid port matches the firmware, and that full shuffle moves every second step 2/3 late

## Firmware patches

`engine/patches` holds the only changes to the firmware sources: accesses past the end of a buffer
that go unnoticed on the STM32 but can hit unmapped memory or produce NaN on a Mac, and the two
hooks ENTROPIA MOD needs:

- `0001`: `diode()` in the master FX distortion read `table_1n914[257]` when the signal clips at 1.0
- `0002`: the saw/tri/rect wavetables get a zero guard row, because the oscillators' interpolation
  reads one sample past the last row
- `0003`: switching the master FX delay from normal to ping-pong while it runs left the buffer
  positions in the whole buffer, and ping-pong wrote up to 22 KB past its end (this crashed Live,
  e.g. when loading a ping-pong kit after one with a normal delay)
- `0004`: the mixer hands each voice's block to the bridge (`lxr_voice_block`), for the ENTROPIA
  MOD sends
- `0005`: a reverb can be given its own buffer (the firmware's shares the FX RAM with the delay;
  ENTROPIA MOD runs both)

## Licence

GPL-3.0-only, like the firmware ([LICENSE](LICENSE)). "Erica Synths" and "LXR" are trademarks of
Erica Synths; ELABS RLX is an independent, unofficial build. The built-in ELABS kits are its own;
Erica Synths' and Hrtl's content isn't included and is imported by the user from their downloads.
