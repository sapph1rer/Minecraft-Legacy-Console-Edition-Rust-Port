// release builds on Windows are GUI programs: no console window next to the game (errors go to `mcview_crash.log`)
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

//! `mcview` — the Minecraft Legacy Console Edition port: title screen, world creation, survival / creative play.
//!
//! ```text
//! mcview [--dim overworld|nether|end|flat] [--seed N] [--radius CHUNKS] [--survival] [--direct]
//!        [--save FILE.ms] [--assets terrain.png] [--no-audio]
//!        [--screenshot out.png [--size WxH] [--screen title|pause|options|inventory|crafting|creative|createworld]
//!                              [--mouse X,Y] [--give ID[:COUNT[:DAMAGE]],...] [--cam X,Y,Z] [--yaw DEG] [--pitch DEG]
//!                              [--hold-attack TICKS] [--time TICKS]]
//! ```
//! Controls: WASD walk, Space jump (double-tap or F = fly), Shift sneak / descend, Ctrl sprint, hold left = break,
//! hold / click right = use or place, 1-9 / wheel = hotbar, E = inventory (creative: item picker), R = crafting,
//! Q = drop item, Esc = pause menu.

use glam::Vec3;
use mc_client::audio::AudioEngine;
use mc_client::game::{creative_inventory, Actions, Game};
use mc_client::inventory::Inventory;
use mc_client::mesh::mesh_chunk;
use mc_client::physics::Input;
use mc_client::render::{create_depth, load_png_rgba, Camera, Renderer};
use mc_client::screens::{Action, Key, Options, Screen, ScreenId};
use mc_client::skyrender::{Rgba, SkyAssets};
use mc_client::ui::{draw_hud, gui_scale, HudState, Tex, UiAssets, UiBuilder, UiFrame};
use mc_client::save::{self, ChunkStore, WorldInfo};
use mc_client::stream::{empty_level, Streamer};
use mc_client::world::{generate, load_save, spawn_point, Dim};
use std::collections::HashSet;
use mc_world::item::ItemStack;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

#[derive(Clone)]
struct Args {
    dim: Dim,
    seed: i64,
    radius: i32,
    save: Option<String>,
    assets: Option<String>,
    screenshot: Option<String>,
    cam: Option<[f32; 3]>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    time: i64,
    survival: bool,
    /// screenshot: hold the attack button for this many ticks before the shot
    hold_attack: u32,
    no_audio: bool,
    /// skip the title screen
    direct: bool,
    size: (u32, u32),
    screen: Option<String>,
    mouse: (f32, f32),
    give: Vec<ItemStack>,
    /// screenshot: items lying on the ground in front of the player
    drops: Vec<ItemStack>,
    /// screenshot: mobs placed in front of the player
    mobs: Vec<(mc_client::mob::MobKind, u32)>,
    no_mobs: bool,
    /// screenshot: a frozen lake in front of the spawn point
    ice_lake: bool,
    /// screenshot: a row of special-shaped blocks in front of the player
    showcase: bool,
    /// screenshot: redstone, orientation and gravity demo
    showcase2: bool,
    /// do not touch options.txt / saves (tests, screenshots)
    no_save: bool,
    /// screenshot: keep chunk streaming on (the camera is moved before the shot)
    stream: bool,
    /// screenshot: start with rain ("rain") or a thunderstorm with a bolt in front of the player ("thunder")
    weather: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args { dim: Dim::Overworld, seed: 666, radius: 10, save: None, assets: None, screenshot: None, cam: None, yaw: None, pitch: None, time: 0, survival: false, hold_attack: 0, no_audio: false, direct: false, size: (1280, 720), screen: None, mouse: (0.0, 0.0), give: Vec::new(), drops: Vec::new(), mobs: Vec::new(), no_mobs: false, ice_lake: false, showcase: false, showcase2: false, no_save: false, stream: false, weather: None };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| panic!("{k} needs a value"));
        match k.as_str() {
            "--dim" => a.dim = Dim::parse(&v()).expect("unknown --dim"),
            "--seed" => a.seed = v().parse().expect("--seed"),
            "--radius" => a.radius = v().parse().expect("--radius"),
            "--save" => a.save = Some(v()),
            "--assets" => a.assets = Some(v()),
            "--screenshot" => a.screenshot = Some(v()),
            "--time" => a.time = v().parse().expect("--time"),
            "--survival" => a.survival = true,
            "--direct" | "--play" => a.direct = true,
            "--hold-attack" => a.hold_attack = v().parse().expect("--hold-attack"),
            "--no-audio" => a.no_audio = true,
            "--screen" => a.screen = Some(v()),
            "--size" => {
                let s = v();
                let (w, h) = s.split_once('x').expect("--size WxH");
                a.size = (w.parse().expect("w"), h.parse().expect("h"));
            }
            "--mouse" => {
                let p: Vec<f32> = v().split(',').map(|s| s.parse().expect("--mouse x,y")).collect();
                a.mouse = (p[0], p[1]);
            }
            "--give" => {
                for part in v().split(',') {
                    let f: Vec<u32> = part.split(':').map(|s| s.parse().expect("--give id[:count[:damage]]")).collect();
                    a.give.push(ItemStack::with_damage(f[0] as u16, *f.get(1).unwrap_or(&1) as u8, *f.get(2).unwrap_or(&0) as u16));
                }
            }
            "--no-mobs" => a.no_mobs = true,
            "--ice-lake" => a.ice_lake = true,
            "--showcase" => a.showcase = true,
            "--showcase2" => a.showcase2 = true,
            "--no-save" => a.no_save = true,
            "--stream" => a.stream = true,
            "--weather" => a.weather = Some(v()),
            "--mob" => {
                use mc_client::mob::MobKind as K;
                for part in v().split(',') {
                    let (n, c) = part.split_once(':').unwrap_or((part, "1"));
                    let kind = match n {
                        "pig" => K::Pig,
                        "cow" => K::Cow,
                        "sheep" => K::Sheep,
                        "chicken" => K::Chicken,
                        "zombie" => K::Zombie,
                        "skeleton" => K::Skeleton,
                        "creeper" => K::Creeper,
                        "spider" => K::Spider,
                        "cavespider" => K::CaveSpider,
                        "slime" => K::Slime,
                        "lavaslime" => K::LavaSlime,
                        _ => panic!("--mob pig|cow|sheep|chicken|zombie|skeleton|creeper|spider|cavespider|slime|lavaslime[:count]"),
                    };
                    a.mobs.push((kind, c.parse().expect("--mob kind:count")));
                }
            }
            "--drop" => {
                for part in v().split(',') {
                    let f: Vec<u32> = part.split(':').map(|s| s.parse().expect("--drop id[:count[:damage]]")).collect();
                    a.drops.push(ItemStack::with_damage(f[0] as u16, *f.get(1).unwrap_or(&1) as u8, *f.get(2).unwrap_or(&0) as u16));
                }
            }
            "--cam" => {
                let p: Vec<f32> = v().split(',').map(|s| s.parse().expect("--cam x,y,z")).collect();
                a.cam = Some([p[0], p[1], p[2]]);
            }
            "--yaw" => a.yaw = Some(v().parse::<f32>().expect("--yaw").to_radians()),
            "--pitch" => a.pitch = Some(v().parse::<f32>().expect("--pitch").to_radians()),
            "-h" | "--help" => {
                println!("see the header of mcview.rs");
                std::process::exit(0);
            }
            o => panic!("unknown argument {o}"),
        }
    }
    a
}

fn find_atlas(arg: &Option<String>) -> (Vec<u8>, PathBuf) {
    let rel = "Minecraft.Client/Common/res/TitleUpdate/res/terrain.png";
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(p) = arg {
        cands.push(p.into());
    }
    // next to the executable first (a packaged game folder keeps `Minecraft.Client/...` beside `mcview.exe`), then up from the cwd
    let mut dirs = vec![std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())), std::env::current_dir().ok()];
    for start in dirs.drain(..) {
        let mut dir = start;
        while let Some(d) = dir {
            cands.push(d.join(rel));
            cands.push(d.join("terrain.png"));
            dir = d.parent().map(|p| p.to_path_buf());
        }
    }
    for c in &cands {
        if let Ok(b) = std::fs::read(c) {
            eprintln!("atlas: {}", c.display());
            return (b, c.clone());
        }
    }
    panic!("terrain.png not found; pass --assets PATH (…/Minecraft.Client/Common/res/TitleUpdate/res/terrain.png)");
}

// ------------------------------------------------------------------------------------------ core

/// Everything that is not the window: renderer, UI state, the running game.
struct Core {
    renderer: Renderer,
    assets: UiAssets,
    opts: Options,
    audio: Option<AudioEngine>,
    game: Option<Game>,
    screen: Option<Screen>,
    args: Args,
    pending_start: Option<(i64, bool)>,
    empty_inv: Inventory,
    mouse_px: (f32, f32),
    left_down: bool,
    quit: bool,
    items_alpha: mc_client::itemmesh::Alpha,
    mob_models: std::cell::RefCell<mc_client::mobrender::MobModels>,
    data_dir: PathBuf,
    streamer: Option<Streamer>,
    world: Option<WorldInfo>,
    pending_load: Option<String>,
    modified: HashSet<(i32, i32)>,
    opts_dirty: bool,
    pool: mc_client::meshpool::MeshPool,
    pending_name: String,
    weather_mesh: Vec<(mc_client::render::EntTex, mc_client::mesh::MeshData)>,
}

impl Core {
    fn new(args: &Args, device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Core {
        let (atlas_bytes, atlas_path) = find_atlas(&args.assets);
        let (mut atlas, aw, ah) = load_png_rgba(&atlas_bytes).expect("decode terrain.png");
        mc_client::chest::paint_icons(&mut atlas, aw);
        mc_client::chest::paint_half_cells(&mut atlas, aw);
        mc_client::chest::paint_particle_tiles(&mut atlas, aw);
        let res = atlas_path.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_default();
        let load = |rels: &[&str]| -> Rgba {
            for r in rels {
                if let Ok(b) = std::fs::read(res.join(r)) {
                    let (px, w, h) = load_png_rgba(&b).unwrap_or_else(|e| panic!("{r}: {e}"));
                    return Rgba { px, w, h };
                }
            }
            panic!("missing sky asset {rels:?} under {}", res.display());
        };
        let sky_assets = SkyAssets {
            sun: load(&["terrain/sun.png", "1_2_2/terrain/sun.png"]),
            moon: load(&["1_2_2/terrain/moon_phases.png", "terrain/moon_phases.png"]),
            clouds: load(&["environment/clouds.png", "1_2_2/environment/clouds.png"]),
            tunnel: load(&["TitleUpdate/res/misc/tunnel.png", "1_2_2/misc/tunnel.png"]),
        };
        let mut renderer = Renderer::new(device, queue, format, &atlas, aw, ah, args.dim.sky());
        renderer.set_sky_assets(&sky_assets);
        // the TitleUpdate resources override the base ones (TU19's items.png carries the later items)
        let read_res = |f: &str| -> std::io::Result<Vec<u8>> {
            let tu = res.join("TitleUpdate/res");
            let alt = if f == "gui/items.png" { "items.png" } else { f };
            // the TU font sheet (Default.png) has another layout than the 16x16 gui font: never override it
            if f.starts_with("font/") {
                return std::fs::read(res.join(f));
            }
            std::fs::read(tu.join(alt)).or_else(|_| std::fs::read(res.join(f)))
        };
        let mut assets = UiAssets::new();
        assets.set_size(Tex::Terrain, aw, ah);
        for t in Tex::ALL {
            let Some(f) = t.file() else { continue };
            match read_res(f) {
                Ok(b) => {
                    let (px, w, h) = load_png_rgba(&b).unwrap_or_else(|e| panic!("{f}: {e}"));
                    assets.set_size(t, w, h);
                    if t == Tex::Font {
                        assets.measure_font(&px, w, h);
                    }
                    renderer.set_ui_texture(t, &px, w, h);
                }
                Err(_) => eprintln!("gui: {f} not found under {} (drawn blank)", res.display()),
            }
        }
        renderer.set_ui_texture(Tex::Extra, &mc_client::ui::hud_extras(), mc_client::ui::EXTRA_W, mc_client::ui::EXTRA_H);
        let mut items_alpha = mc_client::itemmesh::Alpha::default();
        for t in mc_client::render::EntTex::ALL {
            let Some(f) = t.file() else { continue };
            match read_res(f) {
                Ok(b) => {
                    let (px, w, h) = load_png_rgba(&b).unwrap_or_else(|e| panic!("{f}: {e}"));
                    if t == mc_client::render::EntTex::Items {
                        items_alpha = mc_client::itemmesh::Alpha::from_rgba(&px, w, h);
                    }
                    renderer.set_entity_texture(t, &px, w, h);
                }
                Err(_) => eprintln!("entity texture {f} not found under {} (layer skipped)", res.display()),
            }
        }
        let audio = if args.no_audio {
            None
        } else {
            let root = atlas_path.ancestors().map(|a| a.join("Windows64Media/Sound")).find(|p| p.join("Minecraft").is_dir());
            match root {
                Some(r) => {
                    let a = AudioEngine::new(r.clone());
                    eprintln!("sound: {} (output: {})", r.display(), a.device_name.as_deref().unwrap_or("none"));
                    Some(a)
                }
                None => {
                    eprintln!("sound: Windows64Media/Sound not found next to the assets, running silent");
                    None
                }
            }
        };
        let mut c = Core { renderer, assets, opts: Options::default(), audio, game: None, screen: Some(Screen::new(ScreenId::Title)), args: args.clone(), pending_start: None, empty_inv: Inventory::new(), mouse_px: (0.0, 0.0), left_down: false, quit: false, items_alpha, mob_models: Default::default(), data_dir: save::data_dir(), streamer: None, world: None, pending_load: None, modified: HashSet::new(), opts_dirty: false, pool: mc_client::meshpool::MeshPool::new(mc_client::meshpool::MeshPool::default_threads()), pending_name: String::new(), weather_mesh: Vec::new() };
        if c.persist() {
            if let Some(o) = save::load_options(&c.data_dir) {
                c.opts = o;
            }
        }
        if !c.persist() {
            c.opts.render_distance = (args.radius as u32).clamp(2, 16);
        }
        c.apply_options();
        c
    }

    /// Settings and worlds are written to disk (not for screenshots / `--no-save`).
    fn persist(&self) -> bool {
        self.args.screenshot.is_none() && !self.args.no_save
    }

    fn stream_radius(&self) -> i32 {
        (self.opts.render_distance as i32).clamp(2, 16)
    }

    /// Start streaming (and saving) the world that `start_game` just built.
    fn attach_world(&mut self, game: &Game, seed: i64, initial: i32, id: Option<WorldInfo>) {
        if self.args.save.is_some() || (self.args.screenshot.is_some() && !self.args.stream) {
            return;
        }
        let store = id.as_ref().filter(|_| self.persist()).map(|w| ChunkStore::new(save::world_dir(&self.data_dir, &w.id), game.dim));
        self.streamer = Some(Streamer::new(game.dim, seed, &game.level, initial, self.stream_radius(), store));
        self.world = id;
    }

    fn world_info_now(&self) -> Option<WorldInfo> {
        let (g, w) = (self.game.as_ref()?, self.world.as_ref()?);
        Some(WorldInfo {
            day_time: g.day_time,
            game_time: g.level.game_time,
            // a dead player is saved as if respawned: at the world spawn, not where the body fell
            pos: if g.dead { g.spawn_pos } else { [g.player.x, g.player.y, g.player.z] },
            yaw: g.player.yaw,
            pitch: g.player.pitch,
            health: if g.dead { 20 } else { g.health },
            food: if g.dead { 20 } else { g.food },
            flying: g.player.flying,
            spawn: g.spawn_pos,
            inv: g.inv.clone(),
            creative: g.creative,
            last_played: save::now_secs(),
            xp: if g.dead { 0 } else { g.xp_total },
            saturation: g.saturation,
            raining: g.weather.raining,
            thundering: g.weather.thundering,
            rain_time: g.weather.rain_time,
            thunder_time: g.weather.thunder_time,
            ..w.clone()
        })
    }

    /// Write level.txt and the chunks (all of them with `full`, else just the edited ones).
    fn save_game(&mut self, full: bool) {
        if !self.persist() {
            return;
        }
        let Some(info) = self.world_info_now() else { return };
        let _ = save::save_world_info(&self.data_dir, &info);
        if let Some(g) = self.game.as_mut() {
            g.finish_all_pieces();
        }
        if let Some(g) = self.game.as_ref() {
            let text = save::entities_to_text(&g.mobs, &g.entities.items);
            let _ = std::fs::write(save::world_dir(&self.data_dir, &info.id).join(format!("entities_{}.txt", g.dim_name())), text);
        }
        if let (Some(g), Some(s)) = (self.game.as_ref(), self.streamer.as_ref()) {
            if full {
                s.save_all(&g.level, g.level.game_time);
            } else {
                s.save_some(&g.level, g.level.game_time, &self.modified);
            }
        }
        self.modified.clear();
        self.world = Some(info);
    }

    fn flush_options(&mut self) {
        if self.opts_dirty && self.persist() {
            save::save_options(&self.data_dir, &self.opts);
        }
        self.opts_dirty = false;
    }

    /// Everything that must reach the disk before the process ends.
    fn shutdown(&mut self) {
        self.save_game(true);
        self.flush_options();
    }

    /// Play the saved world `id`: chunks come from its save folder (missing ones are generated).
    fn load_world(&mut self, id: &str) {
        let Some(info) = save::list_worlds(&self.data_dir).into_iter().find(|w| w.id == id) else {
            self.screen = Some(Screen::new(ScreenId::Title));
            return;
        };
        let mut lv = empty_level(info.dim, info.seed);
        lv.game_time = info.game_time;
        let store = ChunkStore::new(save::world_dir(&self.data_dir, &info.id), info.dim);
        let mut st = Streamer::new(info.dim, info.seed, &lv, 0, self.stream_radius(), Some(store));
        let centre = ((info.pos[0] / 16.0).floor() as i32, (info.pos[2] / 16.0).floor() as i32);
        let out = st.pump_all(&mut lv, centre, info.game_time);
        self.renderer.clear_chunks();
        for (cx, cz, m) in self.pool.mesh_all(&lv, &out.meshed) {
            self.renderer.set_chunk(cx, cz, &m);
        }
        let mut game = Game::new(lv, info.dim, [info.pos[0] as f32, info.pos[1] as f32, info.pos[2] as f32]);
        game.player.set_pos(info.pos[0], info.pos[1], info.pos[2]);
        game.player.yaw = info.yaw;
        game.player.pitch = info.pitch;
        game.player.flying = info.flying && info.creative;
        game.day_time = info.day_time;
        game.creative = info.creative;
        game.inv = info.inv.clone();
        game.health = info.health;
        game.food = info.food;
        game.last_food = info.food;
        game.saturation = info.saturation;
        game.give_xp(info.xp);
        game.weather.raining = info.raining;
        game.weather.thundering = info.thundering;
        game.weather.rain_time = info.rain_time;
        game.weather.thunder_time = info.thunder_time;
        game.weather.rain_level = if info.raining { 1.0 } else { 0.0 };
        game.weather.o_rain_level = game.weather.rain_level;
        game.weather.thunder_level = if info.thundering { 1.0 } else { 0.0 };
        game.weather.o_thunder_level = game.weather.thunder_level;
        game.spawn_pos = info.spawn;
        game.world_seed = info.seed;
        game.enable_mobs();
        if let Ok(text) = std::fs::read_to_string(save::world_dir(&self.data_dir, &info.id).join(format!("entities_{}.txt", game.dim_name()))) {
            game.mobs.list.clear();
            save::entities_from_text(&text, &mut game);
        }
        self.streamer = Some(st);
        self.world = Some(info);
        self.modified.clear();
        self.game = Some(game);
        self.screen = None;
        self.apply_options();
    }

    /// Per tick: stream chunks around the player, autosave every minute.
    fn stream_tick(&mut self) {
        let rd = self.stream_radius();
        let (Some(g), Some(s)) = (self.game.as_mut(), self.streamer.as_mut()) else { return };
        s.radius = rd;
        let centre = ((g.player.x / 16.0).floor() as i32, (g.player.z / 16.0).floor() as i32);
        let gt = g.level.game_time;
        let out = s.update(&mut g.level, centre, gt, std::time::Duration::from_millis(5));
        for &(cx, cz) in &out.unloaded {
            self.renderer.remove_chunk(cx, cz);
            self.pool.forget(cx, cz);
            self.modified.remove(&(cx, cz));
        }
        // each chunk once per frame (a snapshot copies its 3x3 neighbourhood)
        let mut seen = HashSet::new();
        for &(cx, cz) in out.meshed.iter().chain(out.remesh.iter()) {
            if seen.insert((cx, cz)) {
                self.pool.submit(&g.level, cx, cz);
            }
        }
        if g.ticks > 0 && g.ticks % 1200 == 0 {
            self.save_game(false);
        }
    }

    fn apply_options(&mut self) {
        let rd = if self.streamer.is_some() { self.opts.render_distance } else { self.opts.render_distance.min(self.args.radius.max(2) as u32) };
        self.renderer.fog_end = (rd as f32 - 1.0).max(2.0) * 16.0;
        if let Some(a) = self.audio.as_mut() {
            a.set_effects_volume(self.opts.sound);
        }
    }

    fn start_game(&mut self, seed: i64, creative: bool) {
        let t0 = std::time::Instant::now();
        let a = self.args.clone();
        let lv = match &a.save {
            Some(p) => load_save(&std::fs::read(p).expect("read save"), a.dim, a.radius).expect("load save"),
            None => generate(a.dim, seed, a.radius),
        };
        eprintln!("world ready: {} chunks in {:?}", lv.chunk_count(), t0.elapsed());
        self.renderer.clear_chunks();
        let t1 = std::time::Instant::now();
        let coords: Vec<(i32, i32)> = lv.chunk_coords().collect();
        for (cx, cz, m) in self.pool.mesh_all(&lv, &coords) {
            self.renderer.set_chunk(cx, cz, &m);
        }
        eprintln!("meshed in {:?}, {} triangles", t1.elapsed(), self.renderer.triangle_count());
        let p = spawn_point(&lv, a.dim);
        let mut game = Game::new(lv, a.dim, p);
        game.day_time = a.time;
        game.creative = creative;
        game.inv = if creative { creative_inventory() } else { Inventory::new() };
        for g in &a.give {
            let mut st = *g;
            game.inv.add(&mut st);
        }
        if let Some(c) = a.cam {
            game.player.set_pos(c[0] as f64, c[1] as f64 - 1.62, c[2] as f64);
            game.player.flying = true;
        }
        if let Some(y) = a.yaw {
            game.player.yaw = 180.0 - y.to_degrees();
        }
        if let Some(p) = a.pitch {
            game.player.pitch = -p.to_degrees();
        }
        if a.ice_lake {
            let (px, pz) = (game.player.x.floor() as i32, game.player.z.floor() as i32);
            let gy = game.level.get_top_solid_block(px, pz);
            for x in px - 12..px + 12 {
                for z in pz + 2..pz + 26 {
                    for y in gy - 3..gy + 2 {
                        game.level.set_tile(x, y, z, 0, 0);
                    }
                    game.level.set_tile(x, gy - 3, z, 1, 0);
                    game.level.set_tile(x, gy - 2, z, 9, 0);
                    game.level.set_tile(x, gy - 1, z, 9, 0);
                    game.level.set_tile(x, gy, z, 79, 0);
                }
            }
            game.level.set_tile(px + 3, gy, pz + 6, 9, 0);
            game.level.set_tile(px + 4, gy, pz + 7, 0, 0);
            game.level.change_log.clear();
            let coords: Vec<(i32, i32)> = game.level.chunk_coords().collect();
            for (cx, cz) in coords {
                let m = mesh_chunk(&game.level, cx, cz);
                self.renderer.set_chunk(cx, cz, &m);
            }
        }
        if a.showcase2 {
            use mc_world::tile_ids as t;
            let (px, pz) = (game.player.x.floor() as i32, game.player.z.floor() as i32);
            let gy = game.level.get_top_solid_block(px, pz);
            eprintln!("showcase2 at {px},{gy},{pz}");
            for x in px - 9..px + 17 {
                for z in pz + 2..pz + 17 {
                    for y in gy..gy + 8 {
                        game.level.set_tile(x, y, z, 0, 0);
                    }
                    game.level.set_tile(x, gy - 1, z, t::STONE, 0);
                }
            }
            let mut put = |x: i32, y: i32, z: i32, id: u8, d: u8| {
                game.level.set_tile_and_data(x, y, z, id, d, 0);
            };
            // pistons: one powered from below (pushes a block up), one sticky, TNT, a piston facing east
            put(px - 8, gy, pz + 13, t::REDSTONE_TORCH_ON, 5);
            put(px - 8, gy + 1, pz + 13, t::PISTON_BASE, 1);
            put(px - 8, gy + 2, pz + 13, t::WOOD, 0);
            put(px - 6, gy, pz + 13, t::PISTON_BASE, 5);
            put(px - 5, gy, pz + 13, t::WOOD, 0);
            put(px - 6, gy - 1, pz + 13, t::STONE, 0);
            // chests: a single one, a double along x (facing north) and a double along z (facing east)
            put(px, gy, pz + 5, t::CHEST, 2);
            put(px + 2, gy, pz + 5, t::CHEST, 2);
            put(px + 3, gy, pz + 5, t::CHEST, 2);
            put(px + 6, gy, pz + 5, t::CHEST, 5);
            put(px + 6, gy, pz + 6, t::CHEST, 5);
            put(px - 4, gy, pz + 13, t::TNT, 0);
            put(px - 3, gy, pz + 13, t::TNT, 0);
            put(px - 2, gy, pz + 13, t::PISTON_STICKY_BASE, 3);
            put(px - 2, gy, pz + 14, t::WOOD, 0);
            put(px + 11, gy, pz + 14, t::SIGN, 10);
            put(px + 13, gy, pz + 13, t::STONE, 0);
            put(px + 13, gy, pz + 14, t::WALL_SIGN, 3);
            // chests: one closed, one open
            put(px + 7, gy, pz + 13, t::CHEST, 2);
            put(px + 9, gy, pz + 13, t::CHEST, 5);
            // large chests, one along x (facing north) and one along z (facing east)
            put(px + 15, gy, pz + 13, t::CHEST, 2);
            put(px + 16, gy, pz + 13, t::CHEST, 2);
            put(px + 19, gy, pz + 13, t::CHEST, 5);
            put(px + 19, gy, pz + 14, t::CHEST, 5);
            // double door (left closed, right open, mirrored hinges) and a single open door
            put(px + 2, gy, pz + 13, t::DOOR_WOOD, 1);
            put(px + 2, gy + 1, pz + 13, t::DOOR_WOOD, 8);
            put(px + 3, gy, pz + 13, t::DOOR_WOOD, 1 | 4);
            put(px + 3, gy + 1, pz + 13, t::DOOR_WOOD, 8 | 1);
            put(px + 5, gy, pz + 13, t::DOOR_IRON, 0 | 4);
            put(px + 5, gy + 1, pz + 13, t::DOOR_IRON, 8);
            // stairs: an outer and an inner corner
            put(px - 8, gy, pz + 3, t::STAIRS_WOOD, 0);
            put(px - 7, gy, pz + 3, t::STAIRS_WOOD, 3);
            put(px - 5, gy, pz + 3, t::STAIRS_STONE, 3);
            put(px - 4, gy, pz + 3, t::STAIRS_STONE, 0);
            // fence line, a gate, anvils
            for x in px - 2..=px + 1 {
                put(x, gy, pz + 10, t::FENCE, 0);
            }
            put(px + 2, gy, pz + 10, t::FENCE_GATE, 0);
            put(px + 4, gy, pz + 10, t::ANVIL, 0);
            put(px + 6, gy, pz + 10, t::ANVIL, 1);
            // logs and pistons
            put(px - 8, gy, pz + 5, t::TREE_TRUNK, 4);
            put(px - 6, gy, pz + 5, t::TREE_TRUNK, 8);
            put(px - 4, gy, pz + 5, t::TREE_TRUNK, 0);
            for (i, d) in [2u8, 5, 3, 4, 1, 0].iter().enumerate() {
                put(px - 2 + i as i32, gy, pz + 5, if i % 2 == 0 { t::PISTON_BASE } else { t::PISTON_STICKY_BASE }, *d);
            }
            // redstone: lever -> dust -> repeater -> dust -> lamp, plus a torch inverter and a plate
            put(px - 8, gy, pz + 7, t::LEVER, 5 | 8);
            for x in px - 7..=px - 4 {
                put(x, gy, pz + 7, t::RED_STONE_DUST, 0);
            }
            put(px - 3, gy, pz + 7, t::DIODE_OFF, 3 | (2 << 2));
            for x in px - 2..=px - 1 {
                put(x, gy, pz + 7, t::RED_STONE_DUST, 0);
            }
            put(px, gy, pz + 7, t::REDSTONE_LIGHT, 0);
            put(px - 8, gy, pz + 9, t::LEVER, 5);
            put(px - 7, gy, pz + 9, t::RED_STONE_DUST, 0);
            put(px - 6, gy, pz + 9, t::REDSTONE_LIGHT, 0);
            put(px - 3, gy, pz + 9, t::STONE, 0);
            put(px - 2, gy, pz + 9, t::REDSTONE_TORCH_ON, 1);
            put(px + 2, gy, pz + 9, t::PRESSURE_PLATE_STONE, 1);
            put(px + 3, gy, pz + 9, t::PRESSURE_PLATE_WOOD, 0);
            put(px + 4, gy, pz + 9, t::TORCH, 5);
            put(px + 6, gy, pz + 7, t::SAND, 0);
            put(px + 6, gy + 6, pz + 7, t::SAND, 0);
            // levers / buttons around stone blocks, a ceiling lever, repeaters of every delay/direction, dust shapes
            put(px + 10, gy, pz + 4, t::STONE, 0);
            put(px + 10, gy + 1, pz + 4, t::STONE, 0);
            put(px + 9, gy, pz + 4, t::LEVER, 2);
            put(px + 11, gy, pz + 4, t::LEVER, 1 | 8);
            put(px + 10, gy, pz + 3, t::LEVER, 4);
            put(px + 10, gy, pz + 5, t::LEVER, 3 | 8);
            put(px + 9, gy + 1, pz + 4, t::BUTTON_STONE, 2);
            put(px + 11, gy + 1, pz + 4, t::BUTTON_WOOD, 1 | 8);
            put(px + 10, gy + 1, pz + 3, t::BUTTON_STONE, 4);
            put(px + 10, gy + 1, pz + 5, t::BUTTON_WOOD, 3);
            put(px + 10, gy + 2, pz + 4, t::LEVER, 5);
            put(px + 13, gy, pz + 4, t::LEVER, 6 | 8);
            put(px + 14, gy, pz + 4, t::LEVER, 5);
            for d in 0..4u8 {
                put(px + 9 + d as i32 * 2, gy, pz + 7, t::DIODE_OFF, d | (d << 2));
            }
            put(px + 10, gy, pz + 9, t::RED_STONE_DUST, 0);
            put(px + 11, gy, pz + 9, t::RED_STONE_DUST, 0);
            put(px + 12, gy, pz + 9, t::RED_STONE_DUST, 0);
            put(px + 11, gy, pz + 10, t::RED_STONE_DUST, 0);
            put(px + 14, gy, pz + 9, t::RED_STONE_DUST, 0);
            put(px + 15, gy, pz + 9, t::RED_STONE_DUST, 0);
            put(px + 15, gy, pz + 10, t::RED_STONE_DUST, 0);
            put(px + 15, gy, pz + 8, t::RED_STONE_DUST, 0);
            game.level.change_log.clear();
            for x in px - 9..px + 17 {
                for z in pz + 2..pz + 17 {
                    for y in gy..gy + 3 {
                        game.redstone.notify((x, y, z));
                    }
                }
            }
            game.schedule_fall(px + 6, gy + 6, pz + 7);
            for _ in 0..40 {
                game.tick(&mc_client::physics::Input::default());
            }
            if a.cam.is_none() {
                game.player.set_pos(px as f64 + 0.5, gy as f64 + 8.0, pz as f64 - 1.0);
            }
            if let Ok(v) = std::env::var("SC2_CAM") {
                let c: Vec<f64> = v.split(',').map(|n| n.trim().parse().unwrap()).collect();
                game.player.set_pos(px as f64 + c[0], gy as f64 + c[1], pz as f64 + c[2]);
            }
            game.player.flying = true;
            for (pos, lines) in [((px + 11, gy, pz + 14), ["Hello", "Minecraft", "Rust port", "1:1"]), ((px + 13, gy, pz + 14), ["Wall sign", "", "second line", "end"])] {
                game.level.tile_entities.insert(pos, mc_level::tile_entity::TileEntity::Sign { lines: lines.map(|l| l.to_string()) });
            }
            // the east-facing chest is open
            let cp = (px + 9, gy, pz + 13);
            game.chest_pos = Some(cp);
            game.chest_group = vec![cp, (px + 15, gy, pz + 13), (px + 16, gy, pz + 13)];
            for q in [(px + 15, gy, pz + 13), (px + 16, gy, pz + 13)] {
                game.chest_anim.insert(q, mc_client::chest::ChestAnim { open: 0.7, prev: 0.7 });
                mc_client::chest::set_animating(q, true);
            }
            game.chest_anim.insert(cp, mc_client::chest::ChestAnim { open: 0.7, prev: 0.7 });
            mc_client::chest::set_animating(cp, true);
            let coords: Vec<(i32, i32)> = game.level.chunk_coords().collect();
            for (cx, cz) in coords {
                let m = mesh_chunk(&game.level, cx, cz);
                self.renderer.set_chunk(cx, cz, &m);
            }
        }
        if a.showcase {
            use mc_world::tile_ids as t;
            let (px, pz) = (game.player.x.floor() as i32, game.player.z.floor() as i32);
            let gy = game.level.get_top_solid_block(px, pz);
            eprintln!("showcase at {px},{gy},{pz} player {:.1},{:.1},{:.1} yaw {}", game.player.x, game.player.y, game.player.z, game.player.yaw);
            let row: [(u8, u8); 14] = [(t::STAIRS_WOOD, 0), (t::STAIRS_STONE, 2), (t::STAIRS_BRICKS, 5), (t::DOOR_WOOD, 0), (t::DOOR_IRON, 5), (t::TRAPDOOR, 4), (t::FENCE_GATE, 0), (t::FENCE_GATE, 4), (t::SIGN, 0), (t::LEVER, 6), (t::PRESSURE_PLATE_STONE, 0), (t::DIODE_ON, 1), (t::CAKE, 2), (t::BUTTON_WOOD, 5)];
            for x in px - 8..px + 8 {
                for z in pz + 2..pz + 8 {
                    for y in gy..gy + 4 {
                        game.level.set_tile(x, y, z, 0, 0);
                    }
                    game.level.set_tile(x, gy - 1, z, t::STONE, 0);
                }
            }
            for (i, (id, d)) in row.iter().enumerate() {
                let (x, z) = if i < 3 { (px - 1 + i as i32 * 2 - 2, pz + 3) } else { (px - 7 + i as i32, pz + 5) };
                game.level.set_tile_and_data(x, gy, z, *id, *d, 0);
                if matches!(*id, t::DOOR_WOOD | t::DOOR_IRON) {
                    game.level.set_tile_and_data(x, gy + 1, z, *id, *d | 8, 0);
                }
            }
            game.level.change_log.clear();
            let coords: Vec<(i32, i32)> = game.level.chunk_coords().collect();
            for (cx, cz) in coords {
                let m = mesh_chunk(&game.level, cx, cz);
                self.renderer.set_chunk(cx, cz, &m);
            }
        }
        if !a.no_mobs {
            game.world_seed = seed;
            game.enable_mobs();
        }
        {
            // --mob: place the requested mobs in a row in front of the player
            let (px, pz, yaw) = (game.player.x, game.player.z, (game.player.yaw as f64).to_radians());
            let mut n = 0.0f64;
            for (kind, count) in &a.mobs {
                for _ in 0..*count {
                    let d = 4.0 + (n / 4.0).floor() * 2.5;
                    let side = ((n as i32 % 4) as f64 - 1.5) * 2.2;
                    let (fx, fz) = (-yaw.sin(), yaw.cos());
                    let (x, z) = (px + fx * d + fz * side, pz + fz * d - fx * side);
                    let y = game.level.get_top_solid_block(x.floor() as i32, z.floor() as i32) as f64;
                    let i = game.mobs.spawn(*kind, x, y, z);
                    game.mobs.list[i].b.yaw = (yaw.to_degrees() + 180.0) as f32;
                    game.mobs.list[i].persistent = true;
                    n += 1.0;
                }
            }
        }
        self.modified.clear();
        let info = self.new_world_info(&game, seed, creative);
        self.attach_world(&game, seed, a.radius, info);
        self.game = Some(game);
        self.screen = None;
        self.apply_options();
        self.save_game(false);
    }

    /// The save record of a freshly created world (`None` when nothing is written to disk).
    fn new_world_info(&mut self, g: &Game, seed: i64, creative: bool) -> Option<WorldInfo> {
        if !self.persist() || self.args.save.is_some() {
            return None;
        }
        let name = if self.pending_name.is_empty() { "New World".to_string() } else { std::mem::take(&mut self.pending_name) };
        let id = save::new_world_id(&self.data_dir, &name);
        Some(WorldInfo {
            id,
            name,
            seed,
            dim: g.dim,
            creative,
            day_time: g.day_time,
            game_time: 0,
            pos: [g.player.x, g.player.y, g.player.z],
            yaw: g.player.yaw,
            pitch: g.player.pitch,
            health: g.health,
            food: g.food,
            flying: g.player.flying,
            spawn: g.spawn_pos,
            inv: g.inv.clone(),
            last_played: save::now_secs(),
            xp: g.xp_total,
            saturation: g.saturation,
            raining: g.weather.raining,
            thundering: g.weather.thundering,
            rain_time: g.weather.rain_time,
            thunder_time: g.weather.thunder_time,
        })
    }

    fn quit_to_title(&mut self) {
        self.save_game(true);
        self.streamer = None;
        self.world = None;
        self.game = None;
        self.renderer.clear_chunks();
        self.screen = Some(Screen::new(ScreenId::Title));
    }

    fn gui_size(&self, w: u32, h: u32) -> (f32, f32, f32) {
        let s = gui_scale(w, h);
        ((w as f32 / s).ceil(), (h as f32 / s).ceil(), s)
    }

    /// Close the current screen; a container hands its crafting grid and carried stack back.
    fn close_screen(&mut self) {
        if let (Some(sc), Some(g)) = (self.screen.as_mut(), self.game.as_mut()) {
            if sc.id == ScreenId::Sign {
                if let Some(p) = g.sign_pos.take() {
                    if let Some(mc_level::tile_entity::TileEntity::Sign { lines }) = g.level.tile_entity_mut(p.0, p.1, p.2) {
                        *lines = sc.sign_lines.clone();
                    }
                    g.mark(p.0, p.2);
                }
            }
            if let Some(m) = sc.menu.as_mut() {
                for st in m.close(&mut g.inv) {
                    g.drop_item(st, true);
                }
                if m.kind == mc_client::inventory::MenuKind::Chest {
                    let contents = m.chest_contents();
                    g.close_chest(contents);
                }
            }
        }
        self.screen = None;
    }

    fn open_screen(&mut self, id: ScreenId) {
        let parent = self.screen.as_ref().map(|s| s.id);
        if self.screen.as_ref().map_or(false, |s| s.menu.is_some()) {
            self.close_screen();
        }
        if id == ScreenId::LoadWorld {
            let worlds = save::list_worlds(&self.data_dir);
            if worlds.is_empty() || !self.persist() {
                return self.open_screen(ScreenId::CreateWorld);
            }
            let mut s = Screen::new(id);
            s.set_worlds(worlds);
            self.screen = Some(s);
            return;
        }
        let mut s = Screen::new(id);
        if id == ScreenId::Options {
            s.parent = parent.or(Some(ScreenId::Pause));
        }
        self.screen = Some(s);
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::None => {}
            Action::Close => self.close_screen(),
            Action::StartGame { seed, creative, name } => {
                self.pending_name = name;
                self.pending_start = Some((seed, creative));
            }
            Action::DeleteWorld(id) => {
                save::delete_world(&self.data_dir, &id);
                self.open_screen(ScreenId::LoadWorld);
            }
            Action::QuitToTitle => self.quit_to_title(),
            Action::Respawn => {
                if let Some(g) = self.game.as_mut() {
                    g.respawn();
                }
                self.close_screen();
            }
            Action::QuitApp => self.quit = true,
            Action::Open(id) => {
                // leaving the pause menu for the game
                self.open_screen(id);
            }
            Action::Drop(v) => {
                if let Some(g) = self.game.as_mut() {
                    for st in v {
                        g.drop_item(st, true);
                    }
                }
            }
            Action::OptionsChanged => {
                self.opts_dirty = true;
                self.apply_options();
            }
            Action::LoadWorld(id) => {
                self.pending_load = Some(id);
                self.pending_start = Some((0, false));
            }
        }
    }

    /// Is the world frozen (pause / options opened from the pause menu)?
    fn paused(&self) -> bool {
        self.game.is_some() && self.screen.as_ref().map_or(false, |s| s.is_menu_pause())
    }

    /// Does the mouse belong to the game (captured, no screen)?
    fn wants_grab(&self) -> bool {
        self.game.is_some() && self.screen.is_none() && self.pending_start.is_none()
    }

    // ------------------------------------------------------------------------------------------ input

    fn key_pressed(&mut self, k: KeyCode, text: Option<&str>, ctrl: bool) {
        if let Some(sc) = self.screen.as_mut() {
            let in_container = sc.menu.is_some();
            let action = match k {
                KeyCode::Escape => sc.key(Key::Escape),
                KeyCode::Enter | KeyCode::NumpadEnter => sc.key(Key::Enter),
                KeyCode::Backspace => sc.key(Key::Backspace),
                KeyCode::KeyE | KeyCode::KeyR if in_container && !sc.text_focused() => Action::Close,
                _ => Action::None,
            };
            if let Some(t) = text {
                if (!in_container || sc.text_focused()) && k != KeyCode::Enter && k != KeyCode::Backspace && k != KeyCode::Escape {
                    for c in t.chars() {
                        sc.typed(c);
                    }
                }
            }
            self.apply(action);
            return;
        }
        let Some(g) = self.game.as_mut() else { return };
        let slot = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9].iter().position(|c| *c == k);
        if let Some(i) = slot {
            g.select(i);
        }
        match k {
            KeyCode::Escape => self.screen = Some(Screen::new(ScreenId::Pause)),
            // E: crafting, R: creative inventory (survival: the inventory)
            // (the 3x3 crafting screen is the creative debug key; survival uses the inventory on R and tables)
            KeyCode::KeyE if g.creative => self.screen = Some(Screen::new(ScreenId::Crafting)),
            KeyCode::KeyR => {
                let id = if g.creative { ScreenId::Creative } else { ScreenId::Inventory };
                self.screen = Some(Screen::new(id));
            }
            KeyCode::KeyQ => {
                if let Some(st) = g.inv.selected_item().copied() {
                    // Ctrl+Q throws the whole stack
                    let mut one = st;
                    if !ctrl {
                        one.count = 1;
                        g.inv.use_selected();
                    } else {
                        g.inv.items[g.inv.selected] = None;
                    }
                    g.drop_item(one, false);
                }
            }
            KeyCode::KeyT => g.day_time += 1000,
            KeyCode::KeyY => g.day_time -= 1000,
            KeyCode::KeyP => g.daylight_cycle = !g.daylight_cycle,
            // U: clear → rain → thunderstorm (debug)
            KeyCode::KeyU => g.weather.cycle(),
            KeyCode::KeyF if g.creative => g.player.flying = !g.player.flying,
            _ => {}
        }
    }

    // ------------------------------------------------------------------------------------------ frame

    fn camera(&self, partial: f32) -> Camera {
        let g = self.game.as_ref().unwrap();
        let e = g.eye(partial);
        let pl = &g.player;
        // GameRenderer::getFov: 70 * speed modifier; x 60/70 under water. View bobbing is off while flying.
        let mut fov = pl.fov_degrees(partial) * self.opts.fov / 70.0;
        if g.tile_at_eye_water() {
            fov = fov * 60.0 / 70.0;
        }
        let bob = if pl.flying || !self.opts.view_bobbing { glam::Mat4::IDENTITY } else { pl.bob_view(partial) };
        let eye_post = g.hurt_view(partial) * bob;
        Camera { pos: Vec3::from(e), yaw: std::f32::consts::PI - pl.yaw.to_radians(), pitch: -pl.pitch.to_radians(), fov_y: fov.to_radians(), eye_post }
    }

    /// Hand the sounds of the last ticks to the audio engine and move the listener.
    fn pump_audio(&mut self) {
        let Some(g) = self.game.as_mut() else { return };
        let sounds = g.take_sounds();
        let Some(a) = self.audio.as_mut() else { return };
        let e = g.eye(1.0);
        let f = g.player.look_dir();
        a.set_listener([e[0] as f64, e[1] as f64, e[2] as f64], f);
        for s in &sounds {
            a.play(s);
        }
    }

    /// Upload the chunk meshes the worker threads finished (a few per frame).
    fn collect_meshes(&mut self) {
        let Some(g) = self.game.as_ref() else { return };
        for (cx, cz, m) in self.pool.poll(6) {
            if g.level.has_chunk(cx, cz) {
                self.renderer.set_chunk(cx, cz, &m);
            }
        }
    }

    fn remesh_dirty(&mut self) {
        if let Some(g) = self.game.as_mut() {
            for (cx, cz) in g.take_dirty() {
                self.modified.insert((cx, cz));
                let m = mesh_chunk(&g.level, cx, cz);
                self.renderer.set_chunk(cx, cz, &m);
            }
        }
    }

    /// One 20 Hz tick of everything.
    fn tick(&mut self, input: &Input, act: &Actions) {
        if let Some(sc) = self.screen.as_mut() {
            sc.tick();
        }
        if self.opts_dirty && !self.left_down {
            self.flush_options();
        }
        if self.paused() {
            return;
        }
        let in_screen = self.screen.is_some();
        let Some(g) = self.game.as_mut() else { return };
        if in_screen {
            // a container open: the world keeps running but the player stands still and holds no buttons
            g.tick_with(&Input::default(), &Actions::default());
        } else {
            g.tick_with(input, act);
        }
        if let Some(id) = g.open_screen.take() {
            self.screen = Some(match (id, g.chest_pos) {
                (ScreenId::Chest, Some(_)) => {
                    let items = g.chest_slots();
                    Screen::chest(&items)
                }
                (ScreenId::Sign, _) => {
                    let lines = match g.sign_pos.and_then(|p| g.level.tile_entity_mut(p.0, p.1, p.2)) {
                        Some(mc_level::tile_entity::TileEntity::Sign { lines }) => lines.clone(),
                        _ => Default::default(),
                    };
                    Screen::sign(&lines)
                }
                (ScreenId::Death, _) => Screen::death(g.score),
                _ => Screen::new(id),
            });
        }
        self.pump_audio();
        self.stream_tick();
    }

    /// Targeting, selection outline, particles for this frame.
    fn prepare_world(&mut self, partial: f32) {
        let Some(g) = self.game.as_mut() else { return };
        g.update_target(partial);
        let sel = g.selection_mesh();
        self.renderer.set_overlay(sel.as_ref());
        self.renderer.set_crack(g.crack_mesh().as_ref());
        // rain / snow streaks and lightning bolts
        {
            use mc_client::render::EntTex;
            let cam = g.eye(partial);
            let cam = [cam[0] as f64, cam[1] as f64, cam[2] as f64];
            let rain_level = g.weather.rain(partial);
            let (rain, snow) = mc_client::weather::precipitation_mesh(&mut g.level, cam, g.ticks, partial, rain_level, 10);
            self.weather_mesh.clear();
            if !rain.is_empty() {
                self.weather_mesh.push((EntTex::Rain, rain));
            }
            if !snow.is_empty() {
                self.weather_mesh.push((EntTex::Snow, snow));
            }
            let (row, col) = mc_client::icons::icon("snow").unwrap_or((0, 0));
            let uv = [col as f32 / 16.0 + 0.002, row as f32 / 32.0 + 0.002];
            let mut bolts = mc_client::mesh::MeshData::default();
            for b in &g.weather.bolts {
                let bm = b.mesh(cam, partial, uv);
                let base = bolts.vertices.len() as u32;
                bolts.vertices.extend(bm.vertices);
                bolts.indices.extend(bm.indices.iter().map(|i| i + base));
            }
            if !bolts.is_empty() {
                self.weather_mesh.push((EntTex::Terrain, bolts));
            }
        }
        let pl = &g.player;
        let eye = g.eye(partial);
        let pm = g.particles.build_mesh(&g.level, [eye[0] as f64, eye[1] as f64, eye[2] as f64], pl.yaw, pl.pitch, partial);
        self.renderer.set_particles(Some(&pm));
    }

    fn mob_models_layers(&self, g: &Game, partial: f32) -> Vec<(mc_client::render::EntTex, mc_client::mesh::MeshData)> {
        self.mob_models.borrow_mut().layers(&g.mobs, &g.level, partial)
    }

    /// Dropped items, mobs and the first-person hand for this frame.
    fn entity_frame(&self, cam: &Camera, aspect: f32, partial: f32) -> mc_client::render::EntityFrame {
        let g = self.game.as_ref().unwrap();
        let mut f = mc_client::render::EntityFrame::default();
        f.world = mc_client::entrender::item_layers(&g.entities.items, &g.level, partial, &self.items_alpha);
        f.world.extend(mc_client::entrender::falling_layers(&g.entities.falling, &g.level, partial));
        f.world.extend(mc_client::entrender::chest_layers(&g.chest_anim, &g.level, partial));
        {
            let e = g.eye(partial);
            f.world.extend(mc_client::entrender::sign_layers(&g.level, &self.assets.font_w, self.assets.sizes[Tex::Font as usize], Vec3::from(e)));
        }
        f.world.extend(mc_client::entrender::piston_layers(&g.moving, &g.level, partial));
        f.world.extend(mc_client::entrender::orb_layers(&g.orbs, &g.level, Vec3::from(g.eye(partial)), partial));
        f.world.extend(mc_client::entrender::item_particle_layers(&g.particles, Vec3::from(g.eye(partial)), g.player.yaw, g.player.pitch, partial, &g.level, &self.items_alpha));
        f.world.extend(mc_client::entrender::pearl_layers(&g.pearls, &g.level, Vec3::from(g.eye(partial)), partial, &self.items_alpha));
        f.world.extend(mc_client::entrender::primed_layers(&g.entities.primed, &g.level, partial));
        f.world.extend(self.mob_models_layers(g, partial));
        f.weather = self.weather_mesh.clone();
        let e = g.eye(partial);
        let (s, b) = g.level.tile_light_color(e[0].floor() as i32, e[1].floor() as i32, e[2].floor() as i32);
        let eating = g.inv.selected_item().filter(|st| mc_client::hand::is_food(st)).filter(|_| g.eat_left > 0).map(|_| (g.eat_left as f32, 32.0));
        let p = mc_client::hand::HandParams { hand: &g.hand, partial, pitch: g.player.pitch, yaw: g.player.yaw, light: [s as f32, b as f32], items: &self.items_alpha, eating, invisible: false };
        f.hand = mc_client::hand::build_hand(&p);
        f.hand_proj = Some(glam::Mat4::perspective_rh(self.opts.fov.to_radians(), aspect, 0.05, 1000.0) * cam.eye_post);
        f
    }

    fn build_ui(&self, w: u32, h: u32) -> UiFrame {
        let mut ui = UiBuilder::new(&self.assets, w, h);
        if self.pending_start.is_some() {
            ui.dirt_background();
            ui.text_centered("Generating world...", ui.gw / 2.0, ui.gh / 2.0 - 4.0, 0xFFFFFFFF);
            return ui.finish();
        }
        if let Some(g) = &self.game {
            let hotbar: [Option<&ItemStack>; 9] = std::array::from_fn(|i| g.inv.items[i].as_ref());
            draw_hud(
                &mut ui,
                &HudState {
                    hotbar,
                    selected: g.inv.selected,
                    health: g.health,
                    last_health: g.last_health,
                    food: g.food,
                    last_food: g.last_food,
                    hunger: g.has_effect(mc_client::survival::HUNGER),
                    food_shake: g.saturation <= 0.0,
                    armor: g.inv.armor_value(),
                    air: g.air_hud(),
                    xp_progress: g.xp_progress,
                    xp_level: g.xp_level,
                    show_survival_bars: !g.creative,
                    tick: g.ticks as u32,
                    // `Gui::render`: the bars flash while the hurt invulnerability runs
                    blink: g.invuln >= 10 && (g.invuln / 3) % 2 == 1,
                    tooltip: g.inv.selected_item().filter(|_| g.hl_timer > 0).map(|st| (st.display_name(), ((g.hl_timer as f32 * 256.0 / 10.0) as u32).min(255))),
                },
            );
        }
        if let Some(sc) = &self.screen {
            let inv = self.game.as_ref().map_or(&self.empty_inv, |g| &g.inv);
            sc.draw(&mut ui, &self.opts, inv, self.game.is_some());
        }
        ui.finish()
    }

    fn render(&mut self, view: &wgpu::TextureView, depth: &wgpu::TextureView, w: u32, h: u32, partial: f32) {
        let ui = self.build_ui(w, h);
        if self.game.is_some() && self.pending_start.is_none() {
            self.remesh_dirty();
            self.collect_meshes();
            self.prepare_world(partial);
            let cam = self.camera(partial);
            let env = self.game.as_ref().unwrap().env(partial);
            let ents = self.entity_frame(&cam, w as f32 / h as f32, partial);
            self.renderer.render(view, depth, &cam, w as f32 / h as f32, &ui, &env, &ents);
        } else {
            self.renderer.render_ui_only(view, depth, &ui);
        }
    }

    // ------------------------------------------------------------------------------------------ pointer

    fn pointer_moved(&mut self, w: u32, h: u32, x: f32, y: f32) {
        self.mouse_px = (x, y);
        let (gw, gh, s) = self.gui_size(w, h);
        let down = self.left_down;
        if let Some(sc) = self.screen.as_mut() {
            let a = sc.mouse_moved(gw, gh, x / s, y / s, down, &mut self.opts);
            self.apply(a);
        }
    }

    fn pointer_button(&mut self, w: u32, h: u32, right: bool, pressed: bool, shift: bool) {
        if !right {
            self.left_down = pressed;
        }
        let (gw, gh, s) = self.gui_size(w, h);
        let (x, y) = self.mouse_px;
        if let Some(sc) = self.screen.as_mut() {
            if pressed {
                let empty = &mut self.empty_inv;
                let inv = self.game.as_mut().map_or(empty, |g| &mut g.inv);
                let a = sc.mouse_down(gw, gh, x / s, y / s, right, shift, &mut self.opts, inv);
                self.apply(a);
            } else {
                sc.mouse_released();
            }
        }
    }
}

// ------------------------------------------------------------------------------------------ screenshot

fn screenshot(args: &Args, path: &str) {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false })).expect("no GPU adapter");
    eprintln!("adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: None, required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::default(), memory_hints: Default::default() }, None)).expect("device");
    let fmt = wgpu::TextureFormat::Rgba8Unorm;
    let mut core = Core::new(args, device, queue, fmt);
    let (w, h) = args.size;
    let want = args.screen.as_deref();
    if want != Some("title") && want != Some("createworld") {
        core.start_game(args.seed, !args.survival);
        let g = core.game.as_mut().unwrap();
        if args.cam.is_none() {
            for _ in 0..40 {
                g.tick(&Input::default());
            }
        }
        if !args.drops.is_empty() {
            let mut rng = mc_core::random::Random::with_seed(5);
            let (yaw, p) = ((g.player.yaw as f64).to_radians(), g.player.bb);
            let (fx, fz) = (-yaw.sin(), yaw.cos());
            let n = args.drops.len() as f64;
            for (i, st) in args.drops.iter().enumerate() {
                let side = (i as f64 - (n - 1.0) / 2.0) * 0.8;
                let (x, z) = ((p.x0 + p.x1) / 2.0 + fx * 2.6 - fz * side, (p.z0 + p.z1) / 2.0 + fz * 2.6 + fx * side);
                let mut e = mc_client::entity::ItemEntity::new(x, p.y0 + 1.5, z, *st, &mut rng);
                e.body.xd = 0.0;
                e.body.zd = 0.0;
                e.age = 30;
                g.entities.items.push(e);
            }
            for _ in 0..30 {
                g.tick(&Input::default());
            }
        }
        if args.stream {
            for _ in 0..400 {
                core.stream_tick();
            }
            while core.pool.pending() > 0 {
                core.collect_meshes();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let g = core.game.as_mut().unwrap();
        if let Some(w) = &args.weather {
            g.weather.raining = true;
            g.weather.rain_level = 1.0;
            g.weather.o_rain_level = 1.0;
            g.weather.rain_time = 100000;
            g.weather.thunder_time = 100000;
            if w == "thunder" {
                g.weather.thundering = true;
                g.weather.thunder_level = 1.0;
                g.weather.o_thunder_level = 1.0;
                let yaw = (g.player.yaw as f64).to_radians();
                let (x, z) = (g.player.x - yaw.sin() * 25.0, g.player.z + yaw.cos() * 25.0);
                let y = g.level.get_top_solid_block(x.floor() as i32, z.floor() as i32) as f64;
                g.weather.strike(x, y, z);
            }
        }
        for i in 0..args.hold_attack {
            g.tick_with(&Input::default(), &Actions { attack_pressed: i == 0, attack_held: true, use_held: false });
        }
    }
    core.screen = match want {
        Some("title") => Some(Screen::new(ScreenId::Title)),
        Some("createworld") => {
            let mut s = Screen::new(ScreenId::CreateWorld);
            s.seed_text = "666".into();
            Some(s)
        }
        Some("pause") => Some(Screen::new(ScreenId::Pause)),
        Some("options") => Some(Screen::with_parent(ScreenId::Options, ScreenId::Pause)),
        Some("inventory") => Some(Screen::new(ScreenId::Inventory)),
        Some("crafting") => Some(Screen::new(ScreenId::Crafting)),
        Some("creative") => Some(Screen::new(ScreenId::Creative)),
        Some("sign") => Some(Screen::sign(&["Hello".to_string(), "Minecraft".to_string(), String::new(), String::new()])),
        Some("death") => Some(Screen::death(1234)),
        Some(k) if k == "chest" || k == "chest2" => {
            use mc_level::tile_entity::ItemStack as T;
            let mut c = vec![None; if k == "chest2" { 54 } else { 27 }];
            c[0] = Some(T { id: 264, count: 5, damage: 0, enchantments: vec![] });
            c[1] = Some(T { id: 4, count: 64, damage: 0, enchantments: vec![] });
            c[11] = Some(T { id: 297, count: 3, damage: 0, enchantments: vec![] });
            Some(Screen::chest(&c))
        }
        _ => None,
    };
    let (gw, gh, s) = core.gui_size(w, h);
    if let Some(sc) = core.screen.as_mut() {
        let mut o = core.opts.clone();
        sc.mouse_moved(gw, gh, args.mouse.0, args.mouse.1, false, &mut o);
    }
    let _ = s;
    let tex = core.renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: fmt,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let depth = create_depth(&core.renderer.device, w, h);
    core.render(&view, &depth, w, h, 1.0);
    let buf = core.renderer.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (w * h * 4) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = core.renderer.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::ImageCopyTexture { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::ImageCopyBuffer { buffer: &buf, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    core.renderer.queue.submit(Some(enc.finish()));
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    core.renderer.device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(&data).unwrap();
    }
    std::fs::write(path, out).unwrap();
    eprintln!("wrote {path}");
}

// ------------------------------------------------------------------------------------------ window

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    core: Core,
    keys: std::collections::HashSet<KeyCode>,
    grabbed: bool,
    last: std::time::Instant,
    acc: f32,
    last_space: Option<std::time::Instant>,
    attack_down: bool,
    attack_pressed: bool,
    use_down: bool,
    use_pressed: bool,
    frames: u32,
    fps_t: std::time::Instant,
}

impl State {
    /// Capture / release the cursor so it matches what the core needs.
    fn sync_cursor(&mut self) {
        let want = self.core.wants_grab();
        if want == self.grabbed {
            return;
        }
        if want {
            let ok = self.window.set_cursor_grab(CursorGrabMode::Locked).or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined)).is_ok();
            self.window.set_cursor_visible(!ok);
            self.grabbed = ok;
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
            self.grabbed = false;
        }
    }
}

struct App {
    args: Args,
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let window = Arc::new(el.create_window(Window::default_attributes().with_title("Minecraft Legacy Edition — Rust port").with_inner_size(winit::dpi::LogicalSize::new(1280, 720))).unwrap());
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false })).expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: None, required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::default(), memory_hints: Default::default() }, None)).expect("device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]); // fixed-function GL blended in gamma space
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth = create_depth(&device, config.width, config.height);
        let mut core = Core::new(&self.args, device, queue, format);
        if self.args.direct {
            core.start_game(self.args.seed, !self.args.survival);
        }
        let mut st = State { window, surface, config, depth, core, keys: Default::default(), grabbed: false, last: std::time::Instant::now(), acc: 0.0, last_space: None, attack_down: false, attack_pressed: false, use_down: false, use_pressed: false, frames: 0, fps_t: std::time::Instant::now() };
        st.sync_cursor();
        self.state = Some(st);
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: winit::event::DeviceId, ev: DeviceEvent) {
        if let (Some(s), DeviceEvent::MouseMotion { delta }) = (self.state.as_mut(), ev) {
            if s.grabbed {
                if let Some(g) = s.core.game.as_mut() {
                    // Options::sensitivity → f1^3 * 8 (0.5 gives 1.0)
                    let f1 = s.core.opts.sensitivity * 0.6 + 0.2;
                    let k = f1 * f1 * f1 * 8.0 * 0.15;
                    let p = &mut g.player;
                    p.yaw += delta.0 as f32 * k;
                    p.pitch = (p.pitch + delta.1 as f32 * k).clamp(-90.0, 90.0);
                }
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        let Some(s) = self.state.as_mut() else { return };
        match ev {
            WindowEvent::CloseRequested => {
                s.core.shutdown();
                el.exit();
            }
            WindowEvent::Resized(sz) => {
                s.config.width = sz.width.max(1);
                s.config.height = sz.height.max(1);
                s.surface.configure(&s.core.renderer.device, &s.config);
                s.depth = create_depth(&s.core.renderer.device, s.config.width, s.config.height);
            }
            WindowEvent::CursorMoved { position, .. } => {
                s.core.pointer_moved(s.config.width, s.config.height, position.x as f32, position.y as f32);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(k) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if !s.keys.contains(&k) || event.repeat {
                            if !event.repeat {
                                if k == KeyCode::Space && s.core.screen.is_none() {
                                    let now = std::time::Instant::now();
                                    if s.last_space.map_or(false, |t| (now - t).as_millis() < 300) {
                                        if let Some(g) = s.core.game.as_mut().filter(|g| g.creative) {
                                            g.player.flying = !g.player.flying;
                                        }
                                    }
                                    s.last_space = Some(now);
                                }
                                let text = event.text.as_ref().map(|t| t.to_string());
                                s.core.key_pressed(k, text.as_deref(), s.keys.contains(&KeyCode::ControlLeft));
                            } else if s.core.screen.is_some() && (k == KeyCode::Backspace) {
                                s.core.key_pressed(k, None, false);
                            } else if k == KeyCode::KeyQ && s.core.screen.is_none() {
                                // holding Q keeps throwing items (key repeat)
                                s.core.key_pressed(k, None, false);
                            }
                        }
                        s.keys.insert(k);
                    } else {
                        s.keys.remove(&k);
                    }
                    s.sync_cursor();
                    if s.core.quit {
                        s.core.shutdown();
                        el.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state: st, button, .. } => {
                let pressed = st == ElementState::Pressed;
                if s.core.screen.is_some() {
                    let shift = s.keys.contains(&KeyCode::ShiftLeft) || s.keys.contains(&KeyCode::ShiftRight);
                    match button {
                        MouseButton::Left => s.core.pointer_button(s.config.width, s.config.height, false, pressed, shift),
                        MouseButton::Right => s.core.pointer_button(s.config.width, s.config.height, true, pressed, shift),
                        _ => {}
                    }
                    s.sync_cursor();
                    if s.core.quit {
                        s.core.shutdown();
                        el.exit();
                    }
                } else if s.grabbed {
                    match button {
                        MouseButton::Left => {
                            if pressed {
                                s.attack_pressed = true;
                            }
                            s.attack_down = pressed;
                        }
                        MouseButton::Right => {
                            if pressed {
                                s.use_pressed = true;
                            }
                            s.use_down = pressed;
                        }
                        _ => {}
                    }
                } else if !pressed {
                    s.attack_down = false;
                    s.use_down = false;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 30.0,
                };
                let step = d.signum() as i32 * (d != 0.0) as i32;
                if let Some(sc) = s.core.screen.as_mut() {
                    sc.scroll(step);
                } else if let Some(g) = s.core.game.as_mut() {
                    g.scroll(step);
                }
            }
            WindowEvent::RedrawRequested => {
                let want = if s.core.opts.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
                if s.config.present_mode != want {
                    s.config.present_mode = want;
                    s.surface.configure(&s.core.renderer.device, &s.config);
                }
                let now = std::time::Instant::now();
                let dt = (now - s.last).as_secs_f32().min(0.1);
                s.last = now;
                s.acc += dt;
                if s.core.screen.is_some() {
                    s.attack_down = false;
                    s.use_down = false;
                    s.attack_pressed = false;
                    s.use_pressed = false;
                }
                let down = |k: KeyCode| s.keys.contains(&k);
                let axis = |p: KeyCode, n: KeyCode| (down(p) as i32 - down(n) as i32) as f32;
                let input = Input { forward: axis(KeyCode::KeyW, KeyCode::KeyS), strafe: axis(KeyCode::KeyA, KeyCode::KeyD), jump: down(KeyCode::Space), sneak: down(KeyCode::ShiftLeft), sprint: down(KeyCode::ControlLeft) };
                while s.acc >= 0.05 {
                    s.acc -= 0.05;
                    let act = Actions { attack_pressed: std::mem::take(&mut s.attack_pressed), attack_held: s.attack_down, use_held: s.use_down };
                    s.core.tick(&input, &act);
                }
                s.use_pressed = false;
                s.sync_cursor();
                let partial = if s.core.paused() { 1.0 } else { s.acc / 0.05 };
                match s.surface.get_current_texture() {
                    Ok(frame) => {
                        let view = frame.texture.create_view(&Default::default());
                        let (w, h) = (s.config.width, s.config.height);
                        s.core.render(&view, &s.depth, w, h, partial);
                        frame.present();
                    }
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => s.surface.configure(&s.core.renderer.device, &s.config),
                    Err(_) => {}
                }
                // world creation happens after one frame showing "Generating world..."
                if let Some((seed, creative)) = s.core.pending_start.take() {
                    match s.core.pending_load.take() {
                        Some(id) => s.core.load_world(&id),
                        None => s.core.start_game(seed, creative),
                    }
                    s.sync_cursor();
                    s.last = std::time::Instant::now();
                    s.acc = 0.0;
                }
                s.frames += 1;
                if s.fps_t.elapsed().as_secs_f32() >= 1.0 {
                    let title = match &s.core.game {
                        Some(g) => {
                            let p = g.eye(1.0);
                            format!("Minecraft Legacy Edition — Rust port — {} fps — x {:.0} y {:.0} z {:.0}", s.frames, p[0], p[1], p[2])
                        }
                        None => format!("Minecraft Legacy Edition — Rust port — {} fps", s.frames),
                    };
                    s.window.set_title(&title);
                    s.frames = 0;
                    s.fps_t = std::time::Instant::now();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            // Max Framerate: sleep until the next frame is due (the last stretch is spun for accuracy)
            let cap = s.core.opts.max_fps;
            if cap < mc_client::screens::UNLIMITED_FPS {
                let due = s.last + std::time::Duration::from_secs_f64(1.0 / cap as f64);
                let now = std::time::Instant::now();
                if now < due {
                    if due - now > std::time::Duration::from_millis(2) {
                        el.set_control_flow(ControlFlow::WaitUntil(due - std::time::Duration::from_millis(1)));
                    } else {
                        el.set_control_flow(ControlFlow::Poll);
                    }
                    return;
                }
            }
            el.set_control_flow(ControlFlow::Poll);
            s.window.request_redraw();
        }
    }
}

fn main() {
    // without a console the panic message would vanish: keep it in a file next to the executable
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())).unwrap_or_default();
        let _ = std::fs::write(dir.join("mcview_crash.log"), format!("{info}\n"));
        default_hook(info);
    }));
    let args = parse_args();
    if let Some(p) = args.screenshot.clone() {
        screenshot(&args, &p);
        return;
    }
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App { args, state: None };
    el.run_app(&mut app).unwrap();
}
