//! In-game screens (`Screen` subclasses): title, create-world, pause, options, inventory (E), crafting table (R) and the
//! creative picker. Layouts follow the original `TitleScreen` / `PauseScreen` / `OptionsScreen` / `AbstractContainerScreen`
//! classes; the rendering goes through [`UiBuilder`] in GUI units.

use crate::inventory::{CREATIVE_TABS, Inventory, Menu, MenuKind, SlotKind, POOL_COLS, POOL_ROWS};
use crate::ui::{Tex, UiBuilder};
use mc_world::item::ItemStack;

/// Player-facing settings (`Options`).
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// chunks drawn (fog end = (distance - 1) * 16)
    pub render_distance: u32,
    pub fov: f32,
    pub sensitivity: f32,
    pub sound: f32,
    pub music: f32,
    pub view_bobbing: bool,
    pub clouds: bool,
    /// frame-rate cap (Java's "Max Framerate"): 10..=250 in steps of 10, `UNLIMITED_FPS` = uncapped
    pub max_fps: u32,
    pub vsync: bool,
}

/// The slider's last notch: no frame-rate cap.
pub const UNLIMITED_FPS: u32 = 260;

impl Default for Options {
    fn default() -> Self {
        Options { render_distance: 8, fov: 70.0, sensitivity: 0.5, sound: 1.0, music: 1.0, view_bobbing: true, clouds: true, max_fps: 120, vsync: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenId {
    Title,
    CreateWorld,
    /// the list of saved worlds (Singleplayer)
    LoadWorld,
    Pause,
    Options,
    Inventory,
    Crafting,
    Creative,
    Chest,
    /// the sign text editor (`SignEditScreen`)
    Sign,
    /// "Game over!" with Respawn / Title screen (`DeathScreen`)
    Death,
}

#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    /// back to the game (or to the parent screen if there is one)
    Close,
    StartGame { seed: i64, creative: bool, name: String },
    /// delete the saved world with this folder id
    DeleteWorld(String),
    /// play the saved world with this folder id
    LoadWorld(String),
    QuitToTitle,
    QuitApp,
    /// the death screen's Respawn button
    Respawn,
    Open(ScreenId),
    /// stacks to throw into the world
    Drop(Vec<ItemStack>),
    OptionsChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Enter,
    Escape,
}

struct Btn {
    id: u32,
    x: f32,
    y: f32,
    w: f32,
    label: String,
    enabled: bool,
}

pub struct Screen {
    pub id: ScreenId,
    pub menu: Option<Menu>,
    /// where "Done"/"Cancel" return to
    pub parent: Option<ScreenId>,
    pub seed_text: String,
    pub name_text: String,
    pub search_text: String,
    /// text box with the keyboard: 0 none, 1 world name, 2 seed, 3 creative search
    focus: u8,
    /// the world being asked about in the delete confirmation
    confirm_delete: Option<usize>,
    pub creative_mode: bool,
    pub mouse: (f32, f32),
    pub tick: u32,
    drag_slider: Option<u32>,
    /// saved worlds shown by `LoadWorld`, the highlighted one and the first visible row
    worlds: Vec<crate::save::WorldInfo>,
    selected: Option<usize>,
    first_row: usize,
    pub sign_lines: [String; 4],
    pub sign_line: usize,
    /// the score shown on the death screen
    pub score: i32,
    /// rows of the open chest container (3 single, 6 large)
    pub chest_rows: usize,
}

const WORLD_ROWS: usize = 5;

const SLIDER_BASE: u32 = 100;

impl Screen {
    pub fn new(id: ScreenId) -> Screen {
        let menu = match id {
            ScreenId::Inventory => Some(Menu::inventory()),
            ScreenId::Crafting => Some(Menu::crafting()),
            ScreenId::Creative => Some(Menu::creative()),
            _ => None,
        };
        Screen { id, menu, parent: None, seed_text: String::new(), name_text: "New World".into(), search_text: String::new(), focus: if id == ScreenId::CreateWorld { 1 } else { 0 }, confirm_delete: None, creative_mode: true, mouse: (0.0, 0.0), tick: 0, drag_slider: None, worlds: Vec::new(), selected: None, first_row: 0, sign_lines: Default::default(), sign_line: 0, score: 0, chest_rows: 3 }
    }

    /// The death screen with the player's score.
    pub fn death(score: i32) -> Screen {
        let mut s = Screen::new(ScreenId::Death);
        s.score = score;
        s
    }

    /// The sign editor over the sign's current text.
    pub fn sign(lines: &[String; 4]) -> Screen {
        let mut s = Screen::new(ScreenId::Sign);
        s.sign_lines = lines.clone();
        s
    }

    /// The chest screen over `contents` (the tile entity's 27 slots).
    pub fn chest(contents: &[Option<mc_level::tile_entity::ItemStack>]) -> Screen {
        let mut s = Screen::new(ScreenId::Chest);
        s.menu = Some(Menu::chest(contents));
        s.chest_rows = (contents.len() / 9).clamp(3, 6);
        s
    }

    pub fn set_worlds(&mut self, w: Vec<crate::save::WorldInfo>) {
        self.selected = if w.is_empty() { None } else { Some(0) };
        self.worlds = w;
        self.first_row = 0;
    }

    pub fn with_parent(id: ScreenId, parent: ScreenId) -> Screen {
        let mut s = Screen::new(id);
        s.parent = Some(parent);
        s
    }

    /// Does the game keep running behind the screen (inventory etc. in the original pauses nothing in single player)?
    pub fn is_menu_pause(&self) -> bool {
        matches!(self.id, ScreenId::Pause | ScreenId::Options)
    }

    pub fn tick(&mut self) {
        self.tick += 1;
    }

    // ------------------------------------------------------------------------------------------ layout

    fn buttons(&self, gw: f32, gh: f32, o: &Options) -> Vec<Btn> {
        let mut v = Vec::new();
        let (cx, h4) = (gw / 2.0 - 100.0, (gh / 4.0).floor());
        let b = |id: u32, x: f32, y: f32, w: f32, label: &str| Btn { id, x, y, w, label: label.to_string(), enabled: true };
        match self.id {
            ScreenId::Title => {
                let top = h4 + 24.0 * 2.0;
                v.push(b(1, cx, top, 200.0, "Singleplayer"));
                let mut mp = b(2, cx, top + 24.0, 200.0, "Multiplayer");
                mp.enabled = false;
                v.push(mp);
                let mut tp = b(3, cx, top + 48.0, 200.0, "Texture Packs");
                tp.enabled = false;
                v.push(tp);
                v.push(b(0, cx, top + 72.0 + 12.0, 98.0, "Options..."));
                v.push(b(4, gw / 2.0 + 2.0, top + 72.0 + 12.0, 98.0, "Quit Game"));
            }
            ScreenId::Sign => v.push(b(0, cx, gh / 4.0 + 120.0, 200.0, "Done")),
            ScreenId::Death => {
                v.push(b(0, cx, h4 + 72.0, 200.0, "Respawn"));
                v.push(b(1, cx, h4 + 96.0, 200.0, "Title screen"));
            }
            ScreenId::CreateWorld => {
                v.push(b(2, cx, 150.0_f32.min(gh - 90.0), 200.0, &format!("Game Mode: {}", if self.creative_mode { "Creative" } else { "Survival" })));
                v.push(b(1, gw / 2.0 - 155.0, gh - 28.0, 150.0, "Create New World"));
                v.push(b(0, gw / 2.0 + 5.0, gh - 28.0, 150.0, "Cancel"));
            }
            ScreenId::LoadWorld => {
                for i in 0..WORLD_ROWS {
                    let n = self.first_row + i;
                    if let Some(w) = self.worlds.get(n) {
                        let sel = self.selected == Some(n);
                        let mode = if w.creative { "Creative" } else { "Survival" };
                        let label = format!("{}{} - {}", if sel { "> " } else { "" }, w.name, mode);
                        v.push(b(200 + n as u32, cx - 20.0, 44.0 + i as f32 * 24.0, 240.0, &label));
                    }
                }
                let mut up = b(5, cx - 44.0, 44.0, 20.0, "^");
                up.enabled = self.first_row > 0;
                v.push(up);
                let mut dn = b(6, cx - 44.0, 44.0 + (WORLD_ROWS as f32 - 1.0) * 24.0, 20.0, "v");
                dn.enabled = self.first_row + WORLD_ROWS < self.worlds.len();
                v.push(dn);
                if self.confirm_delete.is_some() {
                    v.clear();
                    v.push(b(7, gw / 2.0 - 155.0, gh / 2.0 + 10.0, 150.0, "Yes, delete it"));
                    v.push(b(0, gw / 2.0 + 5.0, gh / 2.0 + 10.0, 150.0, "No, keep it"));
                    return v;
                }
                let mut play = b(1, gw / 2.0 - 155.0, gh - 52.0, 150.0, "Play Selected World");
                play.enabled = self.selected.is_some();
                v.push(play);
                v.push(b(2, gw / 2.0 + 5.0, gh - 52.0, 150.0, "Create New World"));
                let mut del = b(3, gw / 2.0 - 155.0, gh - 28.0, 150.0, "Delete");
                del.enabled = self.selected.is_some();
                v.push(del);
                v.push(b(0, gw / 2.0 + 5.0, gh - 28.0, 150.0, "Cancel"));
            }
            ScreenId::Pause => {
                let yo = -16.0;
                v.push(b(4, cx, h4 + 24.0 + yo, 200.0, "Back to game"));
                v.push(b(0, cx, h4 + 24.0 * 4.0 + yo, 200.0, "Options..."));
                v.push(b(1, cx, h4 + 24.0 * 5.0 + yo, 200.0, "Save and quit to title"));
            }
            ScreenId::Options => {
                let items = self.option_rows(o);
                for (i, (label, _)) in items.iter().enumerate() {
                    let x = gw / 2.0 - 155.0 + (i % 2) as f32 * 160.0;
                    let y = (gh / 6.0).floor() + 24.0 * (i / 2) as f32;
                    v.push(b(SLIDER_BASE + i as u32, x, y, 150.0, label));
                }
                v.push(b(0, cx, (gh / 6.0).floor() + 168.0, 200.0, "Done"));
            }
            _ => {}
        }
        v
    }

    /// Option rows: caption and, for sliders, the slider value 0..1.
    fn option_rows(&self, o: &Options) -> Vec<(String, Option<f32>)> {
        let pct = |n: &str, v: f32| (format!("{}: {}%", n, (v * 100.0).round() as i32), Some(v));
        let onoff = |n: &str, v: bool| (format!("{}: {}", n, if v { "ON" } else { "OFF" }), None);
        vec![
            (format!("Render Distance: {}", o.render_distance), Some((o.render_distance as f32 - 2.0) / 14.0)),
            (format!("FOV: {}", o.fov.round() as i32), Some((o.fov - 30.0) / 80.0)),
            pct("Sensitivity", o.sensitivity),
            pct("Sound", o.sound),
            pct("Music", o.music),
            onoff("View Bobbing", o.view_bobbing),
            onoff("Clouds", o.clouds),
            (if o.max_fps >= UNLIMITED_FPS { "Max Framerate: Unlimited".to_string() } else { format!("Max Framerate: {} fps", o.max_fps) }, Some((o.max_fps.clamp(10, UNLIMITED_FPS) as f32 - 10.0) / 250.0)),
            onoff("VSync", o.vsync),
        ]
    }

    fn panel_origin(&self, gw: f32, gh: f32) -> (f32, f32, Tex, f32, f32) {
        match self.id {
            ScreenId::Inventory => ((gw - 176.0) / 2.0, (gh - 166.0) / 2.0, Tex::Inventory, 176.0, 166.0),
            ScreenId::Crafting => ((gw - 176.0) / 2.0, (gh - 166.0) / 2.0, Tex::Crafting, 176.0, 166.0),
            ScreenId::Chest => {
                // (the menu is taken out of `self` while a click is handled, so the row count lives here)
                let rows = self.chest_rows as f32;
                let h = 114.0 + rows * 18.0;
                ((gw - 176.0) / 2.0, (gh - h) / 2.0, Tex::Container, 176.0, h)
            }
            _ => ((gw - 195.0) / 2.0, (gh - 150.0) / 2.0 + 10.0, Tex::Gui, 195.0, 150.0),
        }
    }

    fn slot_at(&self, gw: f32, gh: f32, x: f32, y: f32) -> Option<usize> {
        let menu = self.menu.as_ref()?;
        let (px, py, ..) = self.panel_origin(gw, gh);
        menu.slots.iter().position(|s| {
            let (sx, sy) = (px + s.x as f32, py + s.y as f32);
            x >= sx - 1.0 && x < sx + 17.0 && y >= sy - 1.0 && y < sy + 17.0
        })
    }

    fn inside_panel(&self, gw: f32, gh: f32, x: f32, y: f32) -> bool {
        let (px, py, _, w, h) = self.panel_origin(gw, gh);
        x >= px && x < px + w && y >= py && y < py + h
    }

    // ------------------------------------------------------------------------------------------ input

    pub fn mouse_moved(&mut self, gw: f32, gh: f32, x: f32, y: f32, down: bool, o: &mut Options) -> Action {
        self.mouse = (x, y);
        if let Some(id) = self.drag_slider {
            if down {
                return self.set_slider(gw, gh, id, x, o);
            }
            self.drag_slider = None;
        }
        Action::None
    }

    fn set_slider(&mut self, gw: f32, gh: f32, id: u32, x: f32, o: &mut Options) -> Action {
        let rows = self.option_rows(o);
        let i = (id - SLIDER_BASE) as usize;
        if rows.get(i).and_then(|r| r.1).is_none() {
            return Action::None;
        }
        let bx = gw / 2.0 - 155.0 + (i % 2) as f32 * 160.0;
        let v = ((x - (bx + 4.0)) / (150.0 - 8.0)).clamp(0.0, 1.0);
        match i {
            0 => o.render_distance = (2.0 + v * 14.0).round() as u32,
            1 => o.fov = (30.0 + v * 80.0).round(),
            2 => o.sensitivity = v,
            3 => o.sound = v,
            4 => o.music = v,
            7 => o.max_fps = ((10.0 + v * 250.0) / 10.0).round() as u32 * 10,
            _ => {}
        }
        let _ = gh;
        Action::OptionsChanged
    }

    pub fn mouse_released(&mut self) {
        self.drag_slider = None;
    }

    pub fn scroll(&mut self, delta: i32) {
        if let Some(m) = &mut self.menu {
            m.scroll_by(-delta);
        }
    }

    pub fn typed(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        match (self.id, self.focus) {
            (ScreenId::CreateWorld, 1) if self.name_text.chars().count() < 32 => self.name_text.push(c),
            (ScreenId::CreateWorld, 2) if self.seed_text.chars().count() < 32 => self.seed_text.push(c),
            (ScreenId::Sign, _) if self.sign_lines[self.sign_line].chars().count() < 15 => self.sign_lines[self.sign_line].push(c),
            (ScreenId::Creative, 3) if self.search_text.chars().count() < 24 => {
                self.search_text.push(c);
                self.refilter();
            }
            _ => {}
        }
    }

    /// A text box has the keyboard (letters must not trigger hot keys).
    pub fn text_focused(&self) -> bool {
        match self.id {
            ScreenId::CreateWorld | ScreenId::Sign => true,
            ScreenId::Creative => self.focus == 3,
            _ => false,
        }
    }

    fn refilter(&mut self) {
        if let Some(m) = self.menu.as_mut() {
            m.set_search(&self.search_text);
        }
    }

    fn start_action(&self) -> Action {
        let name = self.name_text.trim();
        Action::StartGame { seed: parse_seed(&self.seed_text), creative: self.creative_mode, name: if name.is_empty() { "New World".into() } else { name.to_string() } }
    }

    pub fn key(&mut self, k: Key) -> Action {
        match (self.id, k) {
            (ScreenId::Sign, Key::Backspace) => {
                self.sign_lines[self.sign_line].pop();
                Action::None
            }
            (ScreenId::Sign, Key::Enter) => {
                self.sign_line = (self.sign_line + 1) & 3;
                Action::None
            }
            (ScreenId::CreateWorld, Key::Backspace) => {
                match self.focus {
                    1 => self.name_text.pop(),
                    _ => self.seed_text.pop(),
                };
                Action::None
            }
            (ScreenId::Creative, Key::Backspace) if self.focus == 3 => {
                self.search_text.pop();
                self.refilter();
                Action::None
            }
            (ScreenId::Creative, Key::Escape) if self.focus == 3 => {
                self.focus = 0;
                Action::None
            }
            (ScreenId::Creative, Key::Enter) if self.focus == 3 => {
                self.focus = 0;
                Action::None
            }
            (ScreenId::LoadWorld, Key::Escape) if self.confirm_delete.is_some() => {
                self.confirm_delete = None;
                Action::None
            }
            (ScreenId::CreateWorld, Key::Enter) => self.start_action(),
            (ScreenId::Title, _) | (ScreenId::Death, _) => Action::None,
            (ScreenId::LoadWorld, Key::Escape) => Action::Open(ScreenId::Title),
            (ScreenId::LoadWorld, Key::Enter) => self.selected.and_then(|i| self.worlds.get(i)).map_or(Action::None, |w| Action::LoadWorld(w.id.clone())),
            (ScreenId::CreateWorld, Key::Escape) => Action::Open(ScreenId::Title),
            (ScreenId::Options, Key::Escape) => Action::Open(self.parent.unwrap_or(ScreenId::Pause)),
            (_, Key::Escape) => Action::Close,
            _ => Action::None,
        }
    }

    pub fn mouse_down(&mut self, gw: f32, gh: f32, x: f32, y: f32, right: bool, shift: bool, o: &mut Options, inv: &mut Inventory) -> Action {
        self.mouse = (x, y);
        if let Some(mut menu) = self.menu.take() {
            let act = self.menu_click(&mut menu, gw, gh, x, y, right, shift, inv);
            self.menu = Some(menu);
            return act;
        }
        self.buttons_click(gw, gh, x, y, o)
    }

    fn menu_click(&mut self, menu: &mut Menu, gw: f32, gh: f32, x: f32, y: f32, right: bool, shift: bool, inv: &mut Inventory) -> Action {
        {
            let slot = self.slot_at_static(menu, gw, gh, x, y);
            if self.id == ScreenId::Creative && slot.is_none() {
                let (px, py, ..) = self.panel_origin(gw, gh);
                // category tabs above the panel
                for i in 0..CREATIVE_TABS.len() {
                    let (tx, ty) = (px + i as f32 * 28.0, py - 26.0);
                    if x >= tx && x < tx + 26.0 && y >= ty && y < ty + 28.0 {
                        menu.set_tab(i);
                        return Action::None;
                    }
                }
                // search box
                self.focus = if x >= px + 98.0 && x < px + 172.0 && y >= py + 4.0 && y < py + 16.0 { 3 } else { 0 };
                if self.focus == 3 {
                    return Action::None;
                }
                // scrollbar click
                if x >= px + 175.0 && x < px + 187.0 && y >= py + 20.0 && y < py + 110.0 && menu.max_scroll() > 0 {
                    let f = ((y - py - 20.0) / 90.0).clamp(0.0, 1.0);
                    menu.scroll = (f * menu.max_scroll() as f32).round() as usize;
                    let s = menu.scroll as i32;
                    menu.scroll = 0;
                    menu.scroll_by(s);
                    return Action::None;
                }
            }
            let outside = slot.is_none() && !self.inside_panel(gw, gh, x, y);
            let dropped = menu.click(inv, if outside { None } else { slot }, right, shift);
            if slot.is_none() && !outside {
                return Action::None;
            }
            return if dropped.is_empty() { Action::None } else { Action::Drop(dropped) };
        }
    }

    fn buttons_click(&mut self, gw: f32, gh: f32, x: f32, y: f32, o: &mut Options) -> Action {
        let btns = self.buttons(gw, gh, o);
        if self.id == ScreenId::CreateWorld {
            // click on a text box focuses it
            let bx = gw / 2.0 - 100.0;
            let inside = |by: f32| x >= bx && x < bx + 200.0 && y >= by && y < by + 20.0;
            if inside(46.0) {
                self.focus = 1;
            } else if inside(94.0) {
                self.focus = 2;
            }
        }
        for b in &btns {
            if !b.enabled || x < b.x || x >= b.x + b.w || y < b.y || y >= b.y + 20.0 {
                continue;
            }
            return self.button_clicked(gw, gh, b.id, x, o);
        }
        Action::None
    }

    fn slot_at_static(&self, menu: &Menu, gw: f32, gh: f32, x: f32, y: f32) -> Option<usize> {
        let (px, py, ..) = self.panel_origin(gw, gh);
        menu.slots.iter().position(|s| {
            let (sx, sy) = (px + s.x as f32, py + s.y as f32);
            x >= sx - 1.0 && x < sx + 17.0 && y >= sy - 1.0 && y < sy + 17.0
        })
    }

    fn button_clicked(&mut self, gw: f32, gh: f32, id: u32, x: f32, o: &mut Options) -> Action {
        match (self.id, id) {
            (ScreenId::Title, 1) => Action::Open(ScreenId::LoadWorld),
            (ScreenId::LoadWorld, 0) if self.confirm_delete.is_some() => {
                self.confirm_delete = None;
                Action::None
            }
            (ScreenId::LoadWorld, 0) => Action::Open(ScreenId::Title),
            (ScreenId::LoadWorld, 3) => {
                self.confirm_delete = self.selected;
                Action::None
            }
            (ScreenId::LoadWorld, 7) => {
                let id = self.confirm_delete.take().and_then(|i| self.worlds.get(i)).map(|w| w.id.clone());
                id.map_or(Action::None, Action::DeleteWorld)
            }
            (ScreenId::LoadWorld, 1) => self.selected.and_then(|i| self.worlds.get(i)).map_or(Action::None, |w| Action::LoadWorld(w.id.clone())),
            (ScreenId::LoadWorld, 2) => Action::Open(ScreenId::CreateWorld),
            (ScreenId::LoadWorld, 5) => {
                self.first_row = self.first_row.saturating_sub(1);
                Action::None
            }
            (ScreenId::LoadWorld, 6) => {
                self.first_row = (self.first_row + 1).min(self.worlds.len().saturating_sub(WORLD_ROWS));
                Action::None
            }
            (ScreenId::LoadWorld, n) if n >= 200 => {
                let i = (n - 200) as usize;
                if self.selected == Some(i) {
                    return self.worlds.get(i).map_or(Action::None, |w| Action::LoadWorld(w.id.clone()));
                }
                self.selected = Some(i);
                Action::None
            }
            (ScreenId::Title, 0) => Action::Open(ScreenId::Options),
            (ScreenId::Title, 4) => Action::QuitApp,
            (ScreenId::CreateWorld, 1) => self.start_action(),
            (ScreenId::CreateWorld, 2) => {
                self.creative_mode = !self.creative_mode;
                Action::None
            }
            (ScreenId::CreateWorld, 0) => Action::Open(ScreenId::Title),
            (ScreenId::Sign, 0) => Action::Close,
            (ScreenId::Death, 0) => Action::Respawn,
            (ScreenId::Death, 1) => Action::QuitToTitle,
            (ScreenId::Pause, 4) => Action::Close,
            (ScreenId::Pause, 0) => Action::Open(ScreenId::Options),
            (ScreenId::Pause, 1) => Action::QuitToTitle,
            (ScreenId::Options, 0) => Action::Open(self.parent.unwrap_or(ScreenId::Pause)),
            (ScreenId::Options, n) if n >= SLIDER_BASE => {
                let i = (n - SLIDER_BASE) as usize;
                match i {
                    0..=4 | 7 => {
                        self.drag_slider = Some(n);
                        self.set_slider(gw, gh, n, x, o)
                    }
                    5 => {
                        o.view_bobbing = !o.view_bobbing;
                        Action::OptionsChanged
                    }
                    6 => {
                        o.clouds = !o.clouds;
                        Action::OptionsChanged
                    }
                    8 => {
                        o.vsync = !o.vsync;
                        Action::OptionsChanged
                    }
                    _ => Action::None,
                }
            }
            _ => Action::None,
        }
    }

    // ------------------------------------------------------------------------------------------ drawing

    pub fn draw(&self, ui: &mut UiBuilder, o: &Options, inv: &Inventory, in_game: bool) {
        let (gw, gh) = (ui.gw, ui.gh);
        let (mx, my) = self.mouse;
        match self.id {
            ScreenId::Title => {
                ui.dirt_background();
                let lw = 155.0 + 119.0;
                let lx = (gw / 2.0 - lw / 2.0).floor();
                ui.blit(Tex::McLogo, lx, 30.0, 0.0, 0.0, 155.0, 44.0);
                ui.blit(Tex::McLogo, lx + 155.0, 30.0, 0.0, 45.0, 155.0, 44.0);
                ui.text("Legacy Console Edition (Rust port)", 2.0, 2.0, 0xFF505050);
                let msg = "Copyright Mojang AB. Do not distribute.";
                let w = ui.text_width(msg);
                ui.text(msg, gw - w - 2.0, gh - 10.0, 0xFFFFFFFF);
            }
            ScreenId::CreateWorld => {
                ui.dirt_background();
                ui.text_centered("Create New World", gw / 2.0, 16.0, 0xFFFFFFFF);
                ui.text("World Name", gw / 2.0 - 100.0, 34.0, 0xFFA0A0A0);
                self.draw_edit(ui, gw / 2.0 - 100.0, 46.0, 200.0, &self.name_text, self.focus == 1);
                ui.text("Seed for the World Generator", gw / 2.0 - 100.0, 82.0, 0xFFA0A0A0);
                self.draw_edit(ui, gw / 2.0 - 100.0, 94.0, 200.0, &self.seed_text, self.focus == 2);
                ui.text("Leave blank for a random seed", gw / 2.0 - 100.0, 118.0, 0xFFA0A0A0);
            }
            ScreenId::Sign => {
                // `SignEditScreen`: the dimmed world, the title, a big sign with a post, the lines with the current one bracketed
                ui.dim_background();
                ui.text_centered("Edit sign message:", gw / 2.0, 40.0, 0xFFFFFFFF);
                let (bw, bh) = (144.0f32, 72.0f32);
                let (bx, by) = ((gw / 2.0 - bw / 2.0).floor(), 62.0f32);
                let plank = crate::icons::icon("planks_oak").unwrap_or((4, 0));
                let post = crate::icons::icon("log_oak").unwrap_or((1, 4));
                // post under the board
                let (px0, px1) = (gw / 2.0 - 5.0, gw / 2.0 + 5.0);
                let mut yy = by + bh;
                while yy < by + bh + 40.0 {
                    ui.blit(Tex::Terrain, px0, yy, post.1 as f32 * 16.0 + 3.0, post.0 as f32 * 16.0, px1 - px0, 10.0);
                    yy += 10.0;
                }
                // board: planks tiled, with a dark outline
                ui.fill(bx - 2.0, by - 2.0, bx + bw + 2.0, by + bh + 2.0, 0xFF2A1A08);
                let mut ty = by;
                while ty < by + bh {
                    let mut tx = bx;
                    while tx < bx + bw {
                        let (w, h) = ((bx + bw - tx).min(16.0), (by + bh - ty).min(16.0));
                        ui.blit(Tex::Terrain, tx, ty, plank.1 as f32 * 16.0, plank.0 as f32 * 16.0, w, h);
                        tx += 16.0;
                    }
                    ty += 16.0;
                }
                for i in 0..4 {
                    let mut t = self.sign_lines[i].clone();
                    let cur = i == self.sign_line;
                    if cur {
                        t = if (self.tick / 6) % 2 == 0 { format!("> {}_ <", t) } else { format!("> {}  <", t) };
                    }
                    let sc = 1.5f32;
                    let w = ui.text_width_s(&t, sc);
                    ui.text_plain_s(&t, gw / 2.0 - w / 2.0, by + 6.0 + i as f32 * 16.0, 0xFF000000, sc);
                }
            }
            ScreenId::LoadWorld => {
                ui.dirt_background();
                ui.text_centered("Select World", gw / 2.0, 20.0, 0xFFFFFFFF);
                if let Some(w) = self.confirm_delete.and_then(|i| self.worlds.get(i)) {
                    ui.text_centered("Are you sure you want to delete this world?", gw / 2.0, gh / 2.0 - 30.0, 0xFFFFFFFF);
                    ui.text_centered(&format!("'{}' will be lost forever!", w.name), gw / 2.0, gh / 2.0 - 16.0, 0xFFFF8080);
                }
                if let Some(w) = self.selected.and_then(|i| self.worlds.get(i)) {
                    let info = format!("{} (seed {})", w.id, w.seed);
                    ui.text_centered(&info, gw / 2.0, 44.0 + WORLD_ROWS as f32 * 24.0 + 2.0, 0xFFA0A0A0);
                }
            }
            ScreenId::Death => {
                ui.fill_gradient(0.0, 0.0, gw, gh, 0x60500000, 0xA0803030);
                let (t, sc) = ("Game over!", 2.0);
                let w = ui.text_width_s(t, sc);
                ui.text_plain_s(t, (gw - w) / 2.0, 30.0 * 2.0 / 2.0 + 0.0, 0xFFFFFFFF, sc);
                ui.text_centered(&format!("Score: §e{}", self.score), gw / 2.0, 100.0, 0xFFFFFFFF);
            }
            ScreenId::Pause | ScreenId::Options if in_game => ui.dim_background(),
            ScreenId::Pause | ScreenId::Options => ui.dirt_background(),
            _ => {}
        }
        match self.id {
            ScreenId::Pause => ui.text_centered("Game menu", gw / 2.0, 40.0, 0xFFFFFFFF),
            ScreenId::Options => ui.text_centered("Options", gw / 2.0, 20.0, 0xFFFFFFFF),
            _ => {}
        }
        if self.menu.is_none() {
            let rows = if self.id == ScreenId::Options { self.option_rows(o) } else { Vec::new() };
            for b in self.buttons(gw, gh, o) {
                let hover = b.enabled && mx >= b.x && mx < b.x + b.w && my >= b.y && my < b.y + 20.0;
                if b.id >= SLIDER_BASE && self.id == ScreenId::Options {
                    if let Some((_, Some(v))) = rows.get((b.id - SLIDER_BASE) as usize) {
                        // slider: disabled-looking bar + handle
                        ui.blit_c(Tex::Gui, b.x, b.y, 75.0, 20.0, 0.0, 46.0, 75.0, 20.0, [1.0; 4]);
                        ui.blit_c(Tex::Gui, b.x + 75.0, b.y, 75.0, 20.0, 125.0, 46.0, 75.0, 20.0, [1.0; 4]);
                        let hx = b.x + v.clamp(0.0, 1.0) * (b.w - 8.0);
                        let hv = if hover || self.drag_slider == Some(b.id) { 86.0 } else { 66.0 };
                        ui.blit(Tex::Gui, hx, b.y, 0.0, hv, 4.0, 20.0);
                        ui.blit(Tex::Gui, hx + 4.0, b.y, 196.0, hv, 4.0, 20.0);
                        let c = if hover { 0xFFFFFFA0 } else { 0xFFE0E0E0 };
                        ui.text_centered(&b.label, b.x + b.w / 2.0, b.y + 6.0, c);
                        continue;
                    }
                }
                ui.button(b.x, b.y, b.w, 20.0, &b.label, b.enabled, hover);
            }
            return;
        }
        // container screens
        let menu = self.menu.as_ref().unwrap();
        ui.dim_background();
        let (px, py, tex, pw, ph) = self.panel_origin(gw, gh);
        match menu.kind {
            MenuKind::Inventory => {
                ui.blit(tex, px, py, 0.0, 0.0, pw, ph);
                ui.text_plain("Crafting", px + 86.0, py + 16.0, 0xFF404040);
            }
            MenuKind::Crafting => {
                ui.blit(tex, px, py, 0.0, 0.0, pw, ph);
                ui.text_plain("Crafting", px + 28.0, py + 6.0, 0xFF404040);
                ui.text_plain("Inventory", px + 8.0, py + 72.0, 0xFF404040);
            }
            MenuKind::Creative => self.draw_creative_panel(ui, px, py, menu),
            MenuKind::Chest => {
                // container.png: the chest rows on top, the player inventory part below
                let rows = (menu.chest.len() / 9).clamp(3, 6) as f32;
                let top = rows * 18.0 + 17.0;
                ui.blit(tex, px, py, 0.0, 0.0, pw, top);
                ui.blit(tex, px, py + top, 0.0, 126.0, pw, 96.0);
                ui.text_plain(if rows > 3.0 { "Large Chest" } else { "Chest" }, px + 8.0, py + 6.0, 0xFF404040);
                ui.text_plain("Inventory", px + 8.0, py + ph - 94.0, 0xFF404040);
            }
        }
        let hover = self.slot_at(gw, gh, mx, my);
        for (i, sd) in menu.slots.iter().enumerate() {
            let (sx, sy) = (px + sd.x as f32, py + sd.y as f32);
            if let Some(st) = menu.get(inv, sd.kind) {
                ui.item(&st, sx, sy);
                ui.item_decor(&st, sx, sy);
            }
            if hover == Some(i) {
                ui.fill(sx, sy, sx + 16.0, sy + 16.0, 0x80FFFFFF);
            }
        }
        if self.id == ScreenId::Creative && menu.carried.is_none() {
            if let Some(name) = self.tab_tooltip(gw, gh) {
                self.draw_tooltip_text(ui, name, mx, my);
            }
        }
        if let Some(c) = &menu.carried {
            ui.item(c, mx - 8.0, my - 8.0);
            ui.item_decor(c, mx - 8.0, my - 8.0);
        } else if let Some(i) = hover {
            if let Some(st) = menu.get(inv, menu.slots[i].kind) {
                self.draw_tooltip(ui, &st, mx, my);
            }
        }
    }

    fn draw_edit(&self, ui: &mut UiBuilder, x: f32, y: f32, w: f32, text: &str, focused: bool) {
        ui.fill(x - 1.0, y - 1.0, x + w + 1.0, y + 21.0, 0xFFA0A0A0);
        ui.fill(x, y, x + w, y + 20.0, 0xFF000000);
        let cursor = if focused && (self.tick / 6) % 2 == 0 { "_" } else { "" };
        ui.text(&format!("{}{}", text, cursor), x + 4.0, y + 6.0, 0xFFE0E0E0);
    }

    fn draw_tooltip(&self, ui: &mut UiBuilder, st: &ItemStack, mx: f32, my: f32) {
        let name = st.display_name();
        self.draw_tooltip_text(ui, &name, mx, my);
    }

    fn draw_tooltip_text(&self, ui: &mut UiBuilder, name: &str, mx: f32, my: f32) {
        let w = ui.text_width(name);
        let (x, y) = (mx + 12.0, my - 12.0);
        ui.fill(x - 3.0, y - 4.0, x + w + 3.0, y + 12.0, 0xF0100010);
        ui.fill(x - 3.0, y - 3.0, x + w + 3.0, y + 11.0, 0xF0100010);
        ui.fill_gradient(x - 3.0, y - 3.0, x - 2.0, y + 11.0, 0x505000FF, 0x5028007F);
        ui.fill_gradient(x + w + 2.0, y - 3.0, x + w + 3.0, y + 11.0, 0x505000FF, 0x5028007F);
        ui.text(name, x, y, 0xFFFFFFFF);
    }

    fn draw_creative_panel(&self, ui: &mut UiBuilder, px: f32, py: f32, menu: &Menu) {
        let (w, h) = (195.0, 150.0);
        // category tabs (the selected one is joined to the panel)
        for (i, (_, icon)) in CREATIVE_TABS.iter().enumerate() {
            let (tx, ty) = (px + i as f32 * 28.0, py - 26.0);
            let sel = i == menu.tab;
            let hover = {
                let (mx, my) = self.mouse;
                mx >= tx && mx < tx + 26.0 && my >= ty && my < ty + 28.0
            };
            let (top, body) = if sel { (0xFFFFFFFF, 0xFFC6C6C6) } else if hover { (0xFFE0E0E0, 0xFF9C9C9C) } else { (0xFFB0B0B0, 0xFF8B8B8B) };
            let bottom = if sel { ty + 29.0 } else { ty + 26.0 };
            ui.fill(tx, ty, tx + 26.0, bottom, 0xFF000000);
            ui.fill(tx + 1.0, ty + 1.0, tx + 25.0, bottom - if sel { 0.0 } else { 1.0 }, body);
            ui.fill(tx + 1.0, ty + 1.0, tx + 25.0, ty + 2.0, top);
            ui.fill(tx + 1.0, ty + 1.0, tx + 2.0, bottom - 1.0, top);
            ui.item(&ItemStack::new(*icon, 1), tx + 5.0, ty + 6.0);
            if *icon < 256 {
                // block icons are drawn through `item` as well; nothing else to do
            }
        }
        // panel
        ui.fill(px, py, px + w, py + h, 0xFF000000);
        ui.fill(px + 1.0, py + 1.0, px + w - 1.0, py + h - 1.0, 0xFFFFFFFF);
        ui.fill(px + 2.0, py + 2.0, px + w - 1.0, py + h - 1.0, 0xFF555555);
        ui.fill(px + 2.0, py + 2.0, px + w - 2.0, py + h - 2.0, 0xFFC6C6C6);
        let title = if menu.search.is_empty() { CREATIVE_TABS[menu.tab].0 } else { "Search Items" };
        ui.text_plain(title, px + 8.0, py + 7.0, 0xFF404040);
        // search box
        let (sx, sy) = (px + 98.0, py + 4.0);
        ui.fill(sx, sy, sx + 74.0, sy + 12.0, if self.focus == 3 { 0xFFFFFFFF } else { 0xFF8B8B8B });
        ui.fill(sx + 1.0, sy + 1.0, sx + 73.0, sy + 11.0, 0xFF000000);
        if self.search_text.is_empty() && self.focus != 3 {
            ui.text_plain("Search", sx + 3.0, sy + 2.0, 0xFF707070);
        } else {
            let cur = if self.focus == 3 && (self.tick / 6) % 2 == 0 { "_" } else { "" };
            ui.text_plain(&format!("{}{}", self.search_text, cur), sx + 3.0, sy + 2.0, 0xFFE0E0E0);
        }
        // slot bevels: dark top-left, light bottom-right
        for sd in &menu.slots {
            let (sx, sy) = (px + sd.x as f32 - 1.0, py + sd.y as f32 - 1.0);
            ui.fill(sx, sy, sx + 18.0, sy + 18.0, 0xFFFFFFFF);
            ui.fill(sx, sy, sx + 17.0, sy + 17.0, 0xFF373737);
            ui.fill(sx + 1.0, sy + 1.0, sx + 17.0, sy + 17.0, 0xFF8B8B8B);
        }
        // separator above the hotbar
        ui.fill(px + 7.0, py + 117.0, px + 168.0, py + 118.0, 0xFF8B8B8B);
        ui.fill(px + 7.0, py + 118.0, px + 168.0, py + 119.0, 0xFFFFFFFF);
        // scrollbar groove and handle
        ui.fill(px + 174.0, py + 19.0, px + 188.0, py + 111.0, 0xFF373737);
        ui.fill(px + 175.0, py + 20.0, px + 189.0, py + 112.0, 0xFFFFFFFF);
        ui.fill(px + 175.0, py + 20.0, px + 188.0, py + 111.0, 0xFF8B8B8B);
        let max = menu.max_scroll();
        let f = if max == 0 { 0.0 } else { menu.scroll as f32 / max as f32 };
        let hy = py + 20.0 + f * (90.0 - 15.0);
        let enabled = max > 0;
        let (c1, c2) = if enabled { (0xFFFFFFFF, 0xFFC6C6C6) } else { (0xFFB0B0B0, 0xFF9A9A9A) };
        ui.fill(px + 175.0, hy, px + 188.0, hy + 15.0, 0xFF373737);
        ui.fill(px + 175.0, hy, px + 187.0, hy + 14.0, c1);
        ui.fill(px + 176.0, hy + 1.0, px + 187.0, hy + 14.0, c2);
        let _ = (POOL_COLS, POOL_ROWS, SlotKind::Result);
    }

    /// Tooltip of the creative tab under the mouse.
    fn tab_tooltip(&self, gw: f32, gh: f32) -> Option<&'static str> {
        let (px, py, ..) = self.panel_origin(gw, gh);
        let (mx, my) = self.mouse;
        (0..CREATIVE_TABS.len()).find(|i| {
            let (tx, ty) = (px + *i as f32 * 28.0, py - 26.0);
            mx >= tx && mx < tx + 26.0 && my >= ty && my < ty + 28.0
        })
        .map(|i| CREATIVE_TABS[i].0)
    }
}

/// Seed field → numeric seed: digits are used as is, other text goes through Java's `String.hashCode`; empty = random.
pub fn parse_seed(s: &str) -> i64 {
    let t = s.trim();
    if t.is_empty() {
        return std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as i64);
    }
    if let Ok(n) = t.parse::<i64>() {
        return n;
    }
    let mut h: i32 = 0;
    for u in t.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as i32);
    }
    h as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UiAssets;

    #[test]
    fn seed_parsing_matches_java() {
        assert_eq!(parse_seed("666"), 666);
        assert_eq!(parse_seed("-5"), -5);
        assert_eq!(parse_seed("hello"), 99162322);
    }

    #[test]
    fn pause_menu_buttons_return_the_right_actions() {
        let mut s = Screen::new(ScreenId::Pause);
        let (mut o, mut inv) = (Options::default(), Inventory::new());
        let (gw, gh) = (426.0, 240.0);
        let back = s.mouse_down(gw, gh, gw / 2.0, (gh / 4.0).floor() + 24.0 - 16.0 + 5.0, false, false, &mut o, &mut inv);
        assert_eq!(back, Action::Close);
        let quit = s.mouse_down(gw, gh, gw / 2.0, (gh / 4.0).floor() + 24.0 * 5.0 - 16.0 + 5.0, false, false, &mut o, &mut inv);
        assert_eq!(quit, Action::QuitToTitle);
    }

    #[test]
    fn options_slider_drag_changes_the_value() {
        let mut s = Screen::with_parent(ScreenId::Options, ScreenId::Pause);
        let (mut o, mut inv) = (Options::default(), Inventory::new());
        let (gw, gh) = (426.0, 240.0);
        let (bx, by) = (gw / 2.0 - 155.0_f32, (gh / 6.0_f32).floor());
        // sensitivity slider is row 2 → left column, 2nd row
        let y = by + 24.0 + 5.0;
        let a = s.mouse_down(gw, gh, bx + 4.0 + 71.0 * 2.0 + 0.0, by + 24.0 * 1.0 + 5.0, false, false, &mut o, &mut inv);
        let _ = (a, y);
        assert!(o.fov != 70.0 || o.sensitivity != 0.5 || o.render_distance != 8);
        s.mouse_moved(gw, gh, bx + 4.0, by + 5.0, true, &mut o);
        s.mouse_released();
    }

    #[test]
    fn inventory_screen_click_moves_items_and_outside_click_drops() {
        let mut s = Screen::new(ScreenId::Inventory);
        let (mut o, mut inv) = (Options::default(), Inventory::new());
        inv.items[0] = Some(ItemStack::block(1, 5));
        let (gw, gh) = (426.0, 240.0);
        let (px, py) = ((gw - 176.0) / 2.0, (gh - 166.0) / 2.0);
        // hotbar slot 0 is at (8, 142)
        s.mouse_down(gw, gh, px + 9.0, py + 143.0, false, false, &mut o, &mut inv);
        assert_eq!(s.menu.as_ref().unwrap().carried.unwrap().count, 5);
        let act = s.mouse_down(gw, gh, 2.0, 2.0, false, false, &mut o, &mut inv);
        assert!(matches!(act, Action::Drop(ref v) if v[0].count == 5));
    }

    #[test]
    fn creative_search_filters_by_name_and_clears() {
        let mut s = Screen::new(ScreenId::Creative);
        s.focus = 3;
        for c in "diamond".chars() {
            s.typed(c);
        }
        let m = s.menu.as_ref().unwrap();
        assert!(!m.pool.is_empty() && m.pool.iter().all(|st| st.display_name().to_lowercase().contains("diamond")));
        assert!(m.pool.iter().any(|st| st.id >= 256), "items from other tabs are found too");
        s.key(Key::Backspace);
        s.key(Key::Backspace);
        s.key(Key::Backspace);
        s.key(Key::Backspace);
        s.key(Key::Backspace);
        s.key(Key::Backspace);
        s.key(Key::Backspace);
        assert!(s.menu.as_ref().unwrap().search.is_empty());
        assert!(s.text_focused());
    }

    #[test]
    fn create_world_carries_the_typed_name_and_delete_asks_first() {
        let mut s = Screen::new(ScreenId::CreateWorld);
        s.key(Key::Backspace);
        for c in "Abc".chars() {
            s.typed(c);
        }
        assert!(matches!(s.key(Key::Enter), Action::StartGame { ref name, .. } if name == "New WorlAbc"));
        let mut l = Screen::new(ScreenId::LoadWorld);
        let w = crate::save::WorldInfo::from_text("W", "name=Hi\nseed=1\ndim=flat\npos=0,0,0").unwrap();
        l.set_worlds(vec![w]);
        let (gw, gh) = (320.0, 180.0);
        let o = Options::default();
        assert!(matches!(l.button_clicked(gw, gh, 3, 0.0, &mut o.clone()), Action::None));
        assert!(l.confirm_delete.is_some());
        assert!(matches!(l.button_clicked(gw, gh, 7, 0.0, &mut o.clone()), Action::DeleteWorld(ref id) if id == "W"));
    }

    #[test]
    fn screens_draw_inside_the_window() {
        let a = UiAssets::new();
        let (o, inv) = (Options::default(), Inventory::new());
        for id in [ScreenId::Title, ScreenId::LoadWorld, ScreenId::CreateWorld, ScreenId::Pause, ScreenId::Options, ScreenId::Inventory, ScreenId::Crafting, ScreenId::Creative] {
            let mut ui = UiBuilder::new(&a, 1280, 720);
            Screen::new(id).draw(&mut ui, &o, &inv, true);
            let f = ui.finish();
            assert!(!f.verts.is_empty(), "{id:?}");
        }
    }
}

#[cfg(test)]
mod chest_click_tests {
    use super::*;
    use crate::inventory::Inventory;
    use mc_world::item::ItemStack;

    #[test]
    fn large_chest_clicks_land_on_the_slot_under_the_mouse() {
        let c = vec![None; 54];
        let mut s = Screen::chest(&c);
        let mut o = Options::default();
        let mut inv = Inventory::default();
        inv.items[0] = Some(ItemStack::new(1, 10));
        let (gw, gh) = (480.0, 252.0);
        let (px, py, ..) = s.panel_origin(gw, gh);
        // pick up the stack from the hotbar (inventory slot 0 sits at x 8, y 197 in the 6-row layout), drop it in chest slot 30
        s.mouse_down(gw, gh, px + 8.0 + 8.0, py + 197.0 + 8.0, false, false, &mut o, &mut inv);
        assert!(s.menu.as_ref().unwrap().carried.is_some(), "picked up from the hotbar");
        let (cx, cy) = (px + 8.0 + 3.0 * 18.0 + 8.0, py + 18.0 + 3.0 * 18.0 + 8.0);
        s.mouse_down(gw, gh, cx, cy, false, false, &mut o, &mut inv);
        assert_eq!(s.menu.as_ref().unwrap().chest[3 + 3 * 9], Some(ItemStack::new(1, 10)));
    }
}
