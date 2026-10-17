//! Persistence: `options.txt` (settings) and `saves/<id>/` worlds (level.txt + chunks/*.dat), the way the
//! original keeps `options.txt` and one save per world.
use crate::inventory::{Inventory, INV_SIZE};
use crate::screens::Options;
use crate::world::Dim;
use mc_level::chunk::LevelChunk;
use mc_level::chunk_io::parse_chunk;
use mc_level::chunk_save::write_chunk;
use mc_level::tile_entity::TileEntity;

pub type TilePos = ((i32, i32, i32), TileEntity);
use mc_storage::compression::{zlib_compress, zlib_decompress};
use mc_world::item::ItemStack;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where settings and worlds live (`MCRS_DATA`, else `%APPDATA%/mc-rs`, else `~/.mc-rs`, else `./mc-rs-data`).
pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("MCRS_DATA") {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("APPDATA") {
        return PathBuf::from(p).join("mc-rs");
    }
    if let Ok(p) = std::env::var("HOME") {
        return PathBuf::from(p).join(".mc-rs");
    }
    PathBuf::from("mc-rs-data")
}

fn parse_kv(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------- options

pub fn options_to_text(o: &Options) -> String {
    format!(
        "render_distance={}\nfov={}\nsensitivity={}\nsound={}\nmusic={}\nview_bobbing={}\nclouds={}\nmax_fps={}\nvsync={}\n",
        o.render_distance, o.fov, o.sensitivity, o.sound, o.music, o.view_bobbing, o.clouds, o.max_fps, o.vsync
    )
}

pub fn options_from_text(text: &str) -> Options {
    let m = parse_kv(text);
    let mut o = Options::default();
    let f = |k: &str, d: f32| m.get(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite()).unwrap_or(d);
    let b = |k: &str, d: bool| m.get(k).and_then(|v| v.parse::<bool>().ok()).unwrap_or(d);
    o.render_distance = m.get("render_distance").and_then(|v| v.parse::<u32>().ok()).unwrap_or(o.render_distance).clamp(2, 16);
    o.fov = f("fov", o.fov).clamp(30.0, 110.0);
    o.sensitivity = f("sensitivity", o.sensitivity).clamp(0.0, 1.0);
    o.sound = f("sound", o.sound).clamp(0.0, 1.0);
    o.music = f("music", o.music).clamp(0.0, 1.0);
    o.view_bobbing = b("view_bobbing", o.view_bobbing);
    o.clouds = b("clouds", o.clouds);
    o.max_fps = (m.get("max_fps").and_then(|v| v.parse::<u32>().ok()).unwrap_or(o.max_fps).clamp(10, crate::screens::UNLIMITED_FPS) + 5) / 10 * 10;
    o.vsync = b("vsync", o.vsync);
    o
}

pub fn load_options(dir: &Path) -> Option<Options> {
    std::fs::read_to_string(dir.join("options.txt")).ok().map(|t| options_from_text(&t))
}

pub fn save_options(dir: &Path, o: &Options) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("options.txt"), options_to_text(o));
}

// ---------------------------------------------------------------------------------------------- worlds

/// Everything in `level.txt`.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldInfo {
    pub id: String,
    pub name: String,
    pub seed: i64,
    pub dim: Dim,
    pub creative: bool,
    pub day_time: i64,
    pub game_time: i64,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub health: i32,
    pub food: i32,
    pub flying: bool,
    pub spawn: [f64; 3],
    pub inv: Inventory,
    pub last_played: u64,
    /// total experience, saturation and the weather (`LevelData` rain / thunder state and timers)
    pub xp: i32,
    pub saturation: f32,
    pub raining: bool,
    pub thundering: bool,
    pub rain_time: i32,
    pub thunder_time: i32,
}

fn dim_name(d: Dim) -> &'static str {
    match d {
        Dim::Overworld => "overworld",
        Dim::Nether => "nether",
        Dim::End => "end",
        Dim::Flat => "flat",
    }
}

fn inv_to_text(inv: &Inventory) -> String {
    let mut parts = Vec::new();
    for (i, s) in inv.items.iter().enumerate() {
        if let Some(s) = s {
            parts.push(format!("{i}:{}:{}:{}", s.id, s.count, s.damage));
        }
    }
    for (i, s) in inv.armor.iter().enumerate() {
        if let Some(s) = s {
            parts.push(format!("a{i}:{}:{}:{}", s.id, s.count, s.damage));
        }
    }
    parts.join(",")
}

fn inv_from_text(t: &str, selected: usize) -> Inventory {
    let mut inv = Inventory::default();
    for p in t.split(',').filter(|p| !p.is_empty()) {
        let f: Vec<&str> = p.split(':').collect();
        if f.len() != 4 {
            continue;
        }
        let (Ok(id), Ok(count), Ok(damage)) = (f[1].parse::<u16>(), f[2].parse::<u8>(), f[3].parse::<u16>()) else { continue };
        let st = ItemStack::with_damage(id, count, damage);
        if let Some(a) = f[0].strip_prefix('a') {
            if let Ok(i) = a.parse::<usize>() {
                if i < 4 {
                    inv.armor[i] = Some(st);
                }
            }
        } else if let Ok(i) = f[0].parse::<usize>() {
            if i < INV_SIZE {
                inv.items[i] = Some(st);
            }
        }
    }
    inv.selected = selected.min(8);
    inv
}

impl WorldInfo {
    pub fn to_text(&self) -> String {
        format!(
            "name={}\nseed={}\ndim={}\ncreative={}\nday_time={}\ngame_time={}\npos={},{},{}\nyaw={}\npitch={}\nhealth={}\nfood={}\nflying={}\nspawn={},{},{}\nselected={}\nlast_played={}\nxp={}\nsaturation={}\nraining={}\nthundering={}\nrain_time={}\nthunder_time={}\ninv={}\n",
            self.name.replace('\n', " "),
            self.seed,
            dim_name(self.dim),
            self.creative,
            self.day_time,
            self.game_time,
            self.pos[0], self.pos[1], self.pos[2],
            self.yaw,
            self.pitch,
            self.health,
            self.food,
            self.flying,
            self.spawn[0], self.spawn[1], self.spawn[2],
            self.inv.selected,
            self.last_played,
            self.xp,
            self.saturation,
            self.raining,
            self.thundering,
            self.rain_time,
            self.thunder_time,
            inv_to_text(&self.inv)
        )
    }

    pub fn from_text(id: &str, text: &str) -> Option<WorldInfo> {
        let m = parse_kv(text);
        let v3 = |k: &str| -> Option<[f64; 3]> {
            let p: Vec<f64> = m.get(k)?.split(',').filter_map(|s| s.parse().ok()).collect();
            (p.len() == 3 && p.iter().all(|v| v.is_finite())).then(|| [p[0], p[1], p[2]])
        };
        let pos = v3("pos")?;
        Some(WorldInfo {
            id: id.to_string(),
            name: m.get("name").cloned().unwrap_or_else(|| id.to_string()),
            seed: m.get("seed")?.parse().ok()?,
            dim: Dim::parse(m.get("dim")?)?,
            creative: m.get("creative").map_or(true, |v| v == "true"),
            day_time: m.get("day_time").and_then(|v| v.parse().ok()).unwrap_or(0),
            game_time: m.get("game_time").and_then(|v| v.parse().ok()).unwrap_or(0),
            pos,
            yaw: m.get("yaw").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            pitch: m.get("pitch").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            health: m.get("health").and_then(|v| v.parse().ok()).unwrap_or(20),
            food: m.get("food").and_then(|v| v.parse().ok()).unwrap_or(20),
            flying: m.get("flying").map_or(false, |v| v == "true"),
            spawn: v3("spawn").unwrap_or(pos),
            inv: inv_from_text(m.get("inv").map_or("", |s| s.as_str()), m.get("selected").and_then(|v| v.parse().ok()).unwrap_or(0)),
            last_played: m.get("last_played").and_then(|v| v.parse().ok()).unwrap_or(0),
            xp: m.get("xp").and_then(|v| v.parse().ok()).unwrap_or(0),
            saturation: m.get("saturation").and_then(|v| v.parse().ok()).unwrap_or(5.0),
            raining: m.get("raining").map_or(false, |v| v == "true"),
            thundering: m.get("thundering").map_or(false, |v| v == "true"),
            rain_time: m.get("rain_time").and_then(|v| v.parse().ok()).unwrap_or(0),
            thunder_time: m.get("thunder_time").and_then(|v| v.parse().ok()).unwrap_or(0),
        })
    }
}

pub fn saves_dir(dir: &Path) -> PathBuf {
    dir.join("saves")
}

/// A folder name that is safe on every OS, unique among the existing saves.
pub fn new_world_id(dir: &Path, name: &str) -> String {
    let mut base: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    if base.is_empty() {
        base = "World".into();
    }
    let mut id = base.clone();
    let mut n = 2;
    while saves_dir(dir).join(&id).exists() {
        id = format!("{base}{n}");
        n += 1;
    }
    id
}

pub fn world_dir(dir: &Path, id: &str) -> PathBuf {
    saves_dir(dir).join(id)
}

/// Every readable world, most recently played first.
pub fn list_worlds(dir: &Path) -> Vec<WorldInfo> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(saves_dir(dir)) {
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            if let Ok(t) = std::fs::read_to_string(e.path().join("level.txt")) {
                if let Some(w) = WorldInfo::from_text(&id, &t) {
                    v.push(w);
                }
            }
        }
    }
    v.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(a.id.cmp(&b.id)));
    v
}

pub fn save_world_info(dir: &Path, info: &WorldInfo) -> std::io::Result<()> {
    let wd = world_dir(dir, &info.id);
    std::fs::create_dir_all(wd.join("chunks"))?;
    let tmp = wd.join("level.txt.tmp");
    std::fs::write(&tmp, info.to_text())?;
    std::fs::rename(&tmp, wd.join("level.txt"))
}

pub fn delete_world(dir: &Path, id: &str) {
    if !id.is_empty() && !id.contains(['/', '\\', '.']) {
        let _ = std::fs::remove_dir_all(world_dir(dir, id));
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

// ---------------------------------------------------------------------------------------------- chunks

/// Chunk files of one world and dimension: `chunks/<dim>_<cx>_<cz>.dat` = flag byte + zlib(chunk record).
/// The flag is 1 once the chunk has been decorated (populated) by the generator.
#[derive(Clone, Debug)]
pub struct ChunkStore {
    dir: PathBuf,
    dim: Dim,
}

impl ChunkStore {
    pub fn new(world: PathBuf, dim: Dim) -> ChunkStore {
        let dir = world.join("chunks");
        let _ = std::fs::create_dir_all(&dir);
        ChunkStore { dir, dim }
    }

    fn path(&self, cx: i32, cz: i32) -> PathBuf {
        self.dir.join(format!("{}_{cx}_{cz}.dat", dim_name(self.dim)))
    }

    pub fn has(&self, cx: i32, cz: i32) -> bool {
        self.path(cx, cz).exists()
    }

    /// `(chunk, decorated, tile entities)`
    pub fn read(&self, cx: i32, cz: i32) -> Option<(LevelChunk, bool, Vec<TilePos>)> {
        let b = std::fs::read(self.path(cx, cz)).ok()?;
        let (&flag, rest) = b.split_first()?;
        let raw = zlib_decompress(rest, 8 * 1024 * 1024).ok()?;
        let p = parse_chunk(&raw).ok()?;
        let tes = mc_level::chunk_save::read_tile_entities(&p.nbt_tail);
        (p.chunk.x == cx && p.chunk.z == cz).then(|| (p.chunk, flag == 1, tes))
    }

    pub fn write(&self, chunk: &LevelChunk, decorated: bool, game_time: i64, tes: &[((i32, i32, i32), &TileEntity)]) {
        let raw = write_chunk(chunk, game_time, tes, &[]);
        let mut out = vec![decorated as u8];
        out.extend_from_slice(&zlib_compress(&raw));
        let p = self.path(chunk.x, chunk.z);
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, out).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }
}

// ---------------------------------------------------------------------------------------------- entities

/// `entities.txt`: one line per mob / dropped item (the original stores them in the chunk NBT; this keeps them in one file per dimension).
pub fn entities_to_text(mobs: &crate::mobs::Mobs, items: &[crate::entity::ItemEntity]) -> String {
    let mut out = String::new();
    for m in mobs.list.iter().filter(|m| !m.removed && m.health > 0.0 && m.death_time == 0) {
        out.push_str(&format!(
            "mob {:?} {} {} {} {} {} {} {} {} {} {} {}\n",
            m.kind, m.b.x, m.b.bb.y0, m.b.z, m.b.yaw, m.health, m.age, m.love, m.sheep_color, m.sheared as u8, m.persistent as u8, m.size
        ));
    }
    for e in items.iter().filter(|e| !e.removed) {
        out.push_str(&format!("item {} {} {} {} {} {} {}\n", e.item.id, e.item.count, e.item.damage, e.body.x, e.body.y, e.body.z, e.age));
    }
    out
}

/// Put the saved mobs and items back; entities whose chunk is not loaded are skipped.
pub fn entities_from_text(text: &str, game: &mut crate::game::Game) {
    use crate::mob::MobKind;
    for l in text.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok());
        match f.first().copied() {
            Some("mob") if f.len() >= 12 => {
                let kind = match f[1] {
                    "Pig" => MobKind::Pig,
                    "Cow" => MobKind::Cow,
                    "Sheep" => MobKind::Sheep,
                    "Chicken" => MobKind::Chicken,
                    "Zombie" => MobKind::Zombie,
                    "Skeleton" => MobKind::Skeleton,
                    "Creeper" => MobKind::Creeper,
                    "Spider" => MobKind::Spider,
                    "CaveSpider" => MobKind::CaveSpider,
                    "Slime" => MobKind::Slime,
                    "LavaSlime" => MobKind::LavaSlime,
                    _ => continue,
                };
                let (Some(x), Some(y), Some(z)) = (num(2), num(3), num(4)) else { continue };
                if !game.level.has_chunk((x.floor() as i32) >> 4, (z.floor() as i32) >> 4) {
                    continue;
                }
                let i = game.mobs.spawn(kind, x, y, z);
                let m = &mut game.mobs.list[i];
                if kind.is_slime() {
                    // `Slime::readAdditionalSaveData`: `setSize` (which resets the health) before the saved health is applied
                    m.set_slime_size((num(12).unwrap_or(1.0) as i32).max(1));
                }
                let yaw = num(5).unwrap_or(0.0) as f32;
                m.b.yaw = yaw;
                m.y_rot_o = yaw;
                m.y_body_rot = yaw;
                m.y_body_rot_o = yaw;
                m.y_head_rot = yaw;
                m.y_head_rot_o = yaw;
                m.health = num(6).unwrap_or(m.health as f64) as f32;
                m.age = num(7).unwrap_or(0.0) as i32;
                m.love = num(8).unwrap_or(0.0) as i32;
                m.sheep_color = num(9).unwrap_or(0.0) as u8;
                m.sheared = num(10).unwrap_or(0.0) != 0.0;
                m.persistent = num(11).unwrap_or(0.0) != 0.0;
            }
            Some("item") if f.len() >= 8 => {
                let (Some(id), Some(count), Some(damage), Some(x), Some(y), Some(z), Some(age)) = (num(1), num(2), num(3), num(4), num(5), num(6), num(7)) else { continue };
                if !game.level.has_chunk((x.floor() as i32) >> 4, (z.floor() as i32) >> 4) {
                    continue;
                }
                let st = ItemStack::with_damage(id as u16, count as u8, damage as u16);
                let mut e = crate::entity::ItemEntity::new(x, y, z, st, &mut game.random);
                e.body.xd = 0.0;
                e.body.zd = 0.0;
                e.age = age as i32;
                game.entities.items.push(e);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mcrs_test_{n}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn options_roundtrip_and_garbage_is_clamped() {
        let mut o = Options::default();
        o.render_distance = 12;
        o.fov = 90.0;
        o.view_bobbing = false;
        o.sound = 0.25;
        let back = options_from_text(&options_to_text(&o));
        assert_eq!((back.render_distance, back.fov, back.view_bobbing, back.sound), (12, 90.0, false, 0.25));
        let bad = options_from_text("render_distance=999\nfov=nan\nclouds=maybe\n");
        assert_eq!(bad.render_distance, 16);
        let mut f = Options::default();
        f.max_fps = 260;
        f.vsync = false;
        let back = options_from_text(&options_to_text(&f));
        assert_eq!((back.max_fps, back.vsync), (260, false));
        assert_eq!(options_from_text("max_fps=3\n").max_fps, 10);
        assert_eq!(options_from_text("max_fps=100000\n").max_fps, 260);
        assert_eq!(bad.fov, Options::default().fov);
        assert!(bad.clouds);
    }

    #[test]
    fn world_info_roundtrips_with_inventory() {
        let mut inv = Inventory::default();
        inv.items[0] = Some(ItemStack::new(1, 64));
        inv.items[20] = Some(ItemStack::with_damage(276, 1, 12));
        inv.armor[2] = Some(ItemStack::new(300, 1));
        inv.selected = 3;
        let w = WorldInfo { id: "W".into(), name: "My World".into(), seed: -42, dim: Dim::Nether, creative: false, day_time: 123, game_time: 456, pos: [1.5, 70.0, -3.25], yaw: 10.0, pitch: -5.0, health: 14, food: 9, flying: true, spawn: [0.5, 64.0, 0.5], inv, last_played: 99, xp: 1234, saturation: 2.5, raining: true, thundering: false, rain_time: 777, thunder_time: 4321 };
        let back = WorldInfo::from_text("W", &w.to_text()).unwrap();
        assert_eq!(back, w);
        assert!(WorldInfo::from_text("x", "name=a").is_none());
    }

    #[test]
    fn worlds_are_listed_newest_first_and_deleted() {
        let d = tmpdir("list");
        let mk = |id: &str, t: u64| WorldInfo { id: id.into(), name: id.into(), seed: 1, dim: Dim::Flat, creative: true, day_time: 0, game_time: 0, pos: [0.0; 3], yaw: 0.0, pitch: 0.0, health: 20, food: 20, flying: false, spawn: [0.0; 3], inv: Inventory::default(), last_played: t, xp: 0, saturation: 5.0, raining: false, thundering: false, rain_time: 0, thunder_time: 0 };
        save_world_info(&d, &mk("A", 5)).unwrap();
        save_world_info(&d, &mk("B", 9)).unwrap();
        let l = list_worlds(&d);
        assert_eq!(l.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), ["B", "A"]);
        assert_eq!(new_world_id(&d, "A"), "A2");
        delete_world(&d, "B");
        assert_eq!(list_worlds(&d).len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn chunk_store_roundtrips_blocks_and_flag() {
        let lv = crate::world::generate(Dim::Flat, 1, 3);
        let d = tmpdir("chunk");
        let st = ChunkStore::new(d.clone(), Dim::Flat);
        let c = lv.chunk(0, 0).unwrap();
        st.write(c, true, 7, &[]);
        assert!(st.has(0, 0) && !st.has(1, 0));
        let (back, dec, _) = st.read(0, 0).unwrap();
        assert!(dec);
        for y in 0..8 {
            assert_eq!(back.get_tile(3, y, 4), c.get_tile(3, y, 4));
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn mobs_and_items_survive_a_save() {
        use crate::game::Game;
        use crate::mob::MobKind;
        let lv = crate::world::generate(Dim::Flat, 1, 3);
        let mut g = Game::new(lv, Dim::Flat, [0.5, 8.0, 0.5]);
        let i = g.mobs.spawn(MobKind::Sheep, 3.5, 4.0, 2.5);
        g.mobs.list[i].sheep_color = 5;
        g.mobs.list[i].sheared = true;
        g.mobs.list[i].health = 3.0;
        g.mobs.list[i].persistent = true;
        let mut rng = mc_core::random::Random::with_seed(1);
        let mut e = crate::entity::ItemEntity::new(1.5, 5.0, 1.5, ItemStack::with_damage(276, 1, 9), &mut rng);
        e.age = 77;
        g.entities.items.push(e);
        let text = entities_to_text(&g.mobs, &g.entities.items);
        let lv2 = crate::world::generate(Dim::Flat, 1, 3);
        let mut g2 = Game::new(lv2, Dim::Flat, [0.5, 8.0, 0.5]);
        entities_from_text(&text, &mut g2);
        assert_eq!(g2.mobs.list.len(), 1);
        let m = &g2.mobs.list[0];
        assert_eq!((m.kind, m.sheep_color, m.sheared, m.health, m.persistent), (MobKind::Sheep, 5, true, 3.0, true));
        assert!((m.b.x - 3.5).abs() < 1e-9 && (m.b.bb.y0 - 4.0).abs() < 1e-6);
        assert_eq!(g2.entities.items.len(), 1);
        assert_eq!((g2.entities.items[0].item.id, g2.entities.items[0].item.damage, g2.entities.items[0].age), (276, 9, 77));
    }
}
