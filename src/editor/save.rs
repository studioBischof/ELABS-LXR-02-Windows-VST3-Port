//! SAVE page: saves the current kit (to a slot of the session's project and as an .SND file in the
//! library) or the whole session as an SD card style project folder. Names are edited on the LCD
//! like on the LXR (knob 3 moves the cursor, knob 4 / DATA change the character under it) or typed
//! into the field under the display. The browser saves kits, patterns and projects through here too.

use super::*;
use crate::library;
use crate::seq;

const CHARSET: &[u8] = b" ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_!#";
const NAME_LEN: usize = 8;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum SaveWhat {
    Kit,
    Project,
}

pub(super) struct SaveState {
    pub what: SaveWhat,
    /// project kit slot the kit goes to
    pub slot: usize,
    pub name: [u8; NAME_LEN],
    pub cursor: usize,
    /// result of the last save and until when to show it
    pub message: Option<(String, f64)>,
    /// the typed name as it is in the field (it may end in a space the LCD name can't show)
    text: String,
    /// give the field the keyboard on the next frame
    focus: bool,
    /// kit image to store instead of the current parameters (the MORPH page's "save what you hear")
    baked: Option<Vec<u8>>,
    /// the save was started on the MORPH page: go back to the kit A / kit B display when it worked
    from_morph: bool,
}

impl Default for SaveState {
    fn default() -> Self {
        Self { what: SaveWhat::Kit, slot: 0, name: [b' '; NAME_LEN], cursor: 0, message: None, text: String::new(), focus: false, baked: None, from_morph: false }
    }
}

fn set_name(save: &mut SaveState, name: &str) {
    save.name = [b' '; NAME_LEN];
    for (i, b) in name.bytes().filter(|b| CHARSET.contains(b)).take(NAME_LEN).enumerate() {
        save.name[i] = b;
    }
}

fn name(save: &SaveState) -> String {
    String::from_utf8_lossy(&save.name).trim().to_string()
}

/// Called when the page opens: start from the current kit's or project's name and the slot of the
/// kit the shown pattern uses.
pub(super) fn open(st: &mut State) {
    let kit = st.params.kit_name.lock().map(|n| n.clone()).unwrap_or_default();
    let project = st.params.seq.project_name().unwrap_or_else(|| "MYPROJ".to_string());
    let pattern = st.params.seq.pending.load(Ordering::Acquire) as usize % seq::NUM_PATTERNS;
    let slot = st.params.seq.patterns.lock().map(|p| seq::kit_number(&p[pattern])).unwrap_or(0);
    let save = &mut st.save;
    save.slot = slot.min(seq::NUM_PATTERNS - 1);
    save.cursor = 0;
    save.baked = None;
    save.from_morph = false;
    set_name(save, if save.what == SaveWhat::Kit { &kit } else { &project });
    save.focus = true;
}

/// Opens the SAVE page to store a ready kit image (the MORPH page's knob 3): the name is asked for
/// there and the sound is exactly what was heard with the morph setting.
pub(super) fn prepare_baked(st: &mut State, data: Vec<u8>) {
    st.save.what = SaveWhat::Kit;
    open(st);
    st.save.baked = Some(data);
    st.save.from_morph = true;
}

/// The sound as it is heard right now, morph included: at morph 0 this kit sounds like the current
/// morph setting does (the same interpolation the engine applies every block).
pub(super) fn morphed_kit(params: &LxrParams) -> Vec<u8> {
    let morph = params.morph.value() as u8;
    let kit_b = params.morph_kit_data();
    let mut data = crate::param_table::INIT_KIT.to_vec();
    data.resize(crate::kit_table::KIT_SIZE, 0);
    for (info, p) in crate::param_table::PARAMS.iter().zip(&params.sound) {
        let mut v = p.value();
        if morph > 0 && crate::morphs(info) {
            if let Some(b) = &kit_b {
                let target = crate::kits::kit_value(b, info.index, info.min, info.max, &info.display);
                v = crate::morph_value(p.value() as u8, target as u8, morph) as i32;
            }
        }
        data[info.index as usize] = v as u8;
    }
    data
}

/// The kit slot the shown pattern uses.
pub(super) fn current_slot(st: &State) -> usize {
    let pattern = st.params.seq.pending.load(Ordering::Acquire) as usize % seq::NUM_PATTERNS;
    st.params.seq.patterns.lock().map(|p| seq::kit_number(&p[pattern])).unwrap_or(0).min(seq::NUM_PATTERNS - 1)
}

/// The name field under the display: typing goes straight into the name, Enter saves.
pub(super) fn name_field(ui: &mut Ui, st: &mut State, setter: &ParamSetter, rect: Rect) {
    let lcd = name(&st.save);
    // the knobs changed the name: the field follows
    if st.save.text.trim_end() != lcd {
        st.save.text = lcd;
    }
    let mut text = st.save.text.clone();
    let edit = egui::TextEdit::singleline(&mut text)
        .char_limit(NAME_LEN * 2)
        .font(FontId::monospace(13.0))
        .hint_text("type a name")
        .desired_width(rect.width());
    let resp = ui.put(rect, edit).on_hover_text("type the name; Enter saves");
    if std::mem::take(&mut st.save.focus) {
        select_all(ui, &resp, text.chars().count());
    }
    if resp.changed() {
        // a full 8 character name would swallow the next keystroke: keep the last 8 instead, so the
        // name types over itself like on the hardware
        let mut typed: Vec<char> = text
            .chars()
            .filter(|c| c.is_ascii() && CHARSET.contains(&(*c as u8)))
            .collect();
        if typed.len() > NAME_LEN {
            typed.drain(..typed.len() - NAME_LEN);
        }
        let typed: String = typed.into_iter().collect();
        set_name(&mut st.save, &typed);
        st.save.cursor = typed.len().min(NAME_LEN - 1);
        st.save.text = typed;
    }
    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        let now = ui.input(|i| i.time);
        save(st, setter, now);
    }
}

/// The current sound as a kit: the plugin's parameter values over the init kit (for the few
/// values the plugin doesn't expose).
fn current_kit(params: &LxrParams) -> Vec<u8> {
    let mut data = crate::param_table::INIT_KIT.to_vec();
    data.resize(crate::kit_table::KIT_SIZE, 0);
    for (info, p) in crate::param_table::PARAMS.iter().zip(&params.sound) {
        data[info.index as usize] = p.value() as u8;
    }
    data
}

/// Saves the current sound as kit `name`: into project slot `slot` (the shown pattern now uses it)
/// and into the library.
pub(super) fn save_kit(st: &mut State, name: &str, slot: usize) -> std::io::Result<String> {
    // a kit prepared on the MORPH page is stored as it is, otherwise the sound as it is set now
    let data = st.save.baked.take().unwrap_or_else(|| current_kit(&st.params));
    let seq_state = &st.params.seq;
    if let Ok(mut kits) = seq_state.project_kits.lock() {
        kits[slot] = Some((name.to_string(), data.clone()));
    }
    let pattern = seq_state.pending.load(Ordering::Acquire) as usize % seq::NUM_PATTERNS;
    if let Ok(mut p) = seq_state.patterns.lock() {
        seq::set_kit_number(&mut p[pattern], slot);
    }
    seq_state.mark_dirty(pattern);
    if let Ok(mut kit_name) = st.params.kit_name.lock() {
        *kit_name = name.to_string();
    }
    let path = library::save_kit(name, &data)?;
    // the browser should list it right away
    st.kits.push(Kit { bank: "USER".to_string(), name: name.to_string(), data });
    Ok(format!("saved {}", path.file_name().unwrap_or_default().to_string_lossy()))
}

/// Saves the session's patterns and kits as project `name`.
pub(super) fn save_project(st: &mut State, name: &str) -> std::io::Result<String> {
    let seq_state = &st.params.seq;
    let patterns = seq_state.patterns.lock().map(|p| p.clone()).unwrap_or_default();
    let kits = seq_state.project_kits.lock().map(|k| k.clone()).unwrap_or_default();
    let path = library::save_project(name, &patterns, &kits)?;
    if let Ok(mut p) = seq_state.project.lock() {
        *p = Some(name.to_string());
    }
    reload_projects(st);
    Ok(format!("saved Projects/{}", path.file_name().unwrap_or_default().to_string_lossy()))
}

/// Adds the shown pattern, with the sound now playing as its kit, to project folder `project`.
pub(super) fn save_pattern(st: &mut State, project: &str) -> std::io::Result<String> {
    let nr = st.params.seq.pending.load(Ordering::Acquire) as usize % seq::NUM_PATTERNS;
    let pattern = st.params.seq.patterns.lock().map(|p| p[nr].clone()).unwrap_or_default();
    if pattern.is_empty() || seq::is_empty(&pattern) {
        return Err(std::io::Error::other(format!("pattern {} is empty", nr + 1)));
    }
    let kit_name = st.params.kit_name.lock().map(|n| n.clone()).unwrap_or_default();
    let kit = (if kit_name.is_empty() { "KIT".to_string() } else { kit_name }, current_kit(&st.params));
    let (dir, slot) = library::save_pattern(project, &pattern, &kit)?;
    reload_projects(st);
    Ok(format!("saved P{:02} as {}/P{:02}", nr + 1, dir.file_name().unwrap_or_default().to_string_lossy(), slot + 1))
}

/// The pattern page's and the browser's project list, read again after a save.
pub(super) fn reload_projects(st: &mut State) {
    let selected = st.projects.get(st.proj_sel).map(|p| p.name.clone());
    st.projects = seq::all_projects();
    st.proj_sel = selected.and_then(|n| st.projects.iter().position(|p| p.name == n)).unwrap_or(0);
}

fn save(st: &mut State, setter: &ParamSetter, now: f64) {
    let name = name(&st.save);
    let name = if name.is_empty() { "NONAME".to_string() } else { name };
    let result = match st.save.what {
        SaveWhat::Kit => save_kit(st, &name, st.save.slot),
        SaveWhat::Project => save_project(st, &name),
    };
    let (text, ok) = match result {
        Ok(text) => (text, true),
        Err(e) => (format!("not saved: {e}"), false),
    };
    st.save.message = Some((text.clone(), now + 4.0));
    // a kit baked on the MORPH page holds the sound as it was heard: the morph goes back to 0 (a
    // second morph on top of the baked values would move the sound again), the new kit becomes the
    // loaded one, and the display shows kit A and kit B again
    if ok && std::mem::take(&mut st.save.from_morph) {
        setter.begin_set_parameter(&st.params.morph);
        setter.set_parameter(&st.params.morph, 0);
        setter.end_set_parameter(&st.params.morph);
        if let Some(i) = st.kits.iter().rposition(|k| k.name == name) {
            kits::load(setter, &st.params, &st.kits[i]);
            st.kit_index = i;
        }
        st.page = Page::Morph;
        history::say(st, text, now);
    }
}

pub(super) fn lcd_rows(st: &State) -> [String; 2] {
    let s = &st.save;
    let row0 = match s.what {
        SaveWhat::Kit => format!("SAVE KIT {:>7}", format!("slot{:02}", s.slot)),
        SaveWhat::Project => "SAVE PROJECT    ".to_string(),
    };
    // the character under the cursor is shown as '_' when it is a space
    let mut name: Vec<char> = s.name.iter().map(|&b| b as char).collect();
    if name[s.cursor] == ' ' {
        name[s.cursor] = '_';
    }
    [row0, format!("{}{:>8}", name.iter().collect::<String>(), "ok?")]
}

pub(super) fn caption(st: &State, now: f64) -> String {
    match &st.save.message {
        Some((text, until)) if now < *until => text.clone(),
        _ => match st.save.what {
            SaveWhat::Kit => "Enter / DATA saves · knob 1 KIT/PROJ · 2 slot".to_string(),
            SaveWhat::Project => "Enter / DATA saves · knob 1 KIT/PROJ".to_string(),
        },
    }
}

fn change_char(save: &mut SaveState, steps: i32) {
    let cur = CHARSET.iter().position(|&c| c == save.name[save.cursor]).unwrap_or(0) as i32;
    let next = (cur + steps).rem_euclid(CHARSET.len() as i32) as usize;
    save.name[save.cursor] = CHARSET[next];
}

pub(super) fn knob(ui: &mut Ui, st: &mut State, column: usize, center: Pos2, radius: f32, resp: &Response) {
    let s = &st.save;
    let char_pos = CHARSET.iter().position(|&c| c == s.name[s.cursor]).unwrap_or(0);
    let turn = match column {
        0 => Some(if s.what == SaveWhat::Kit { 0.0 } else { 1.0 }),
        1 => (s.what == SaveWhat::Kit).then(|| s.slot as f32 / 63.0),
        2 => Some(s.cursor as f32 / (NAME_LEN - 1) as f32),
        _ => Some(char_pos as f32 / (CHARSET.len() - 1) as f32),
    };
    knob_face(ui.painter(), center, radius, turn, false);
    let scroll = scroll_steps(ui, resp, &mut st.scroll_accum);
    let px = match column {
        0 | 2 => 20.0,
        _ => 8.0,
    };
    let steps = drag_steps(resp, &mut st.drag_accum, px) + scroll;
    if steps == 0 && !(column == 0 && resp.clicked()) {
        return;
    }
    match column {
        0 => {
            st.save.what = if st.save.what == SaveWhat::Kit { SaveWhat::Project } else { SaveWhat::Kit };
            open(st);
        }
        1 => st.save.slot = (st.save.slot as i32 + steps).clamp(0, seq::NUM_PATTERNS as i32 - 1) as usize,
        2 => st.save.cursor = (st.save.cursor as i32 + steps).clamp(0, NAME_LEN as i32 - 1) as usize,
        _ => change_char(&mut st.save, steps),
    }
}

pub(super) fn encoder(ui: &mut Ui, st: &mut State, setter: &ParamSetter, center: Pos2, radius: f32, resp: &Response) {
    let char_pos = CHARSET.iter().position(|&c| c == st.save.name[st.save.cursor]).unwrap_or(0);
    knob_face(ui.painter(), center, radius, Some(char_pos as f32 / (CHARSET.len() - 1) as f32), true);
    let scroll = scroll_steps(ui, resp, &mut st.scroll_accum);
    let steps = drag_steps(resp, &mut st.drag_accum, 8.0) + scroll;
    if steps != 0 {
        change_char(&mut st.save, steps);
    }
    if resp.clicked() {
        let now = ui.input(|i| i.time);
        save(st, setter, now);
    }
}

pub(super) fn click_display(st: &mut State, setter: &ParamSetter, now: f64) {
    save(st, setter, now);
}
