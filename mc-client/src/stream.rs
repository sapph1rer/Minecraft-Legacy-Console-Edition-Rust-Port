//! Chunk streaming: the world around the player is generated (or loaded from the save) on demand, decorated,
//! lit and handed to the mesher; far chunks are written to the save and dropped (`ServerChunkCache` /
//! `MultiPlayerChunkCache` in the original).
use crate::save::{ChunkStore, TilePos};
use crate::world::{generate, Dim};
use mc_level::chunk::LevelChunk;
use mc_level::level::Level;
use mc_worldgen::level_source::{RandomLevelSource, WorldParams};
use mc_worldgen::post_process::Structures;
use mc_worldgen::{end, flat, nether};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What the main thread keeps of a generator: only what decoration needs (the terrain source lives on the workers).
enum Gen {
    Over(Box<Structures>),
    Nether(Box<nether::NetherStructures>),
    End,
    Flat(Box<flat::FlatLevelSource>),
}

/// The terrain half of a generator; one per worker thread.
enum Terrain {
    Over(Box<RandomLevelSource>),
    Nether(Box<nether::HellRandomLevelSource>),
    End(Box<end::TheEndLevelRandomLevelSource>),
    Flat(Box<flat::FlatLevelSource>),
}

impl Terrain {
    fn new(dim: Dim, seed: i64, xz: i32) -> Terrain {
        match dim {
            Dim::Overworld => {
                let mut wp = WorldParams::new(seed);
                wp.xz_size = crate::world::INFINITE_XZ;
                Terrain::Over(Box::new(RandomLevelSource::new(wp)))
            }
            Dim::Nether => Terrain::Nether(Box::new(nether::HellRandomLevelSource::new(nether::NetherParams::new(seed, crate::world::INFINITE_XZ)))),
            Dim::End => Terrain::End(Box::new(end::TheEndLevelRandomLevelSource::new(seed))),
            Dim::Flat => Terrain::Flat(Box::new(flat::FlatLevelSource::new(seed, xz, true))),
        }
    }

    fn chunk(&mut self, c: (i32, i32)) -> LevelChunk {
        let data = match self {
            Terrain::Over(s) => s.get_chunk(c.0, c.1),
            Terrain::Nether(s) => s.get_chunk(c.0, c.1),
            Terrain::End(s) => s.get_chunk(c.0, c.1),
            Terrain::Flat(s) => s.get_chunk(c.0, c.1),
        };
        let mut chunk = LevelChunk::from_blocks(c.0, c.1, &data.blocks);
        chunk.biomes.copy_from_slice(&data.biomes[..256]);
        chunk.recalc_heightmap(false);
        chunk
    }
}

#[derive(Default, Debug)]
pub struct StreamOut {
    /// chunks that just became complete: mesh them
    pub meshed: Vec<(i32, i32)>,
    /// already meshed neighbours whose light changed
    pub remesh: Vec<(i32, i32)>,
    /// chunks removed from the level: drop their meshes
    pub unloaded: Vec<(i32, i32)>,
}

/// A chunk on its way to the disk. Kept in `Shared::pending` until written so a quick turn-around can still load it.
struct WriteJob {
    chunk: LevelChunk,
    tes: Vec<TilePos>,
    time: i64,
}

type Pending = Arc<Mutex<HashMap<(i32, i32), Arc<WriteJob>>>>;

struct Loaded {
    c: (i32, i32),
    chunk: LevelChunk,
    decorated: bool,
    tes: Vec<TilePos>,
}

/// Terrain generation, save reads and save writes run on their own threads, so the frame loop never waits for the disk
/// or the noise generators (`ServerChunkCache` does the same with its worker threads on the console).
struct Workers {
    jobs: Option<Sender<(i32, i32)>>,
    results: Receiver<Loaded>,
    writes: Option<Sender<((i32, i32), Arc<WriteJob>)>>,
    pending: Pending,
    threads: Vec<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
}

impl Workers {
    fn start(dim: Dim, seed: i64, xz: i32, store: Option<ChunkStore>, n: usize) -> Workers {
        let (jtx, jrx) = channel::<(i32, i32)>();
        let jrx = Arc::new(Mutex::new(jrx));
        let (rtx, results) = channel::<Loaded>();
        let (wtx, wrx) = channel::<((i32, i32), Arc<WriteJob>)>();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let mut threads = Vec::new();
        for _ in 0..n.max(1) {
            let (jrx, rtx, store, pending) = (jrx.clone(), rtx.clone(), store.clone(), pending.clone());
            threads.push(std::thread::spawn(move || {
                let mut terrain = Terrain::new(dim, seed, xz);
                loop {
                    let c = match jrx.lock().unwrap().recv() {
                        Ok(c) => c,
                        Err(_) => break,
                    };
                    let queued = pending.lock().unwrap().get(&c).cloned();
                    let loaded = if let Some(j) = queued {
                        Some(Loaded { c, chunk: j.chunk.clone(), decorated: true, tes: j.tes.clone() })
                    } else {
                        store.as_ref().and_then(|st| st.read(c.0, c.1)).map(|(chunk, decorated, tes)| Loaded { c, chunk, decorated, tes })
                    };
                    let l = loaded.unwrap_or_else(|| Loaded { c, chunk: terrain.chunk(c), decorated: false, tes: Vec::new() });
                    if rtx.send(l).is_err() {
                        break;
                    }
                }
            }));
        }
        let writer = store.map(|st| {
            let pending = pending.clone();
            std::thread::spawn(move || {
                while let Ok((c, job)) = wrx.recv() {
                    let tes: Vec<_> = job.tes.iter().map(|(p, t)| (*p, t)).collect();
                    st.write(&job.chunk, true, job.time, &tes);
                    let mut p = pending.lock().unwrap();
                    if p.get(&c).map_or(false, |cur| Arc::ptr_eq(cur, &job)) {
                        p.remove(&c);
                    }
                }
            })
        });
        Workers { jobs: Some(jtx), results, writes: Some(wtx), pending, threads, writer }
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        self.jobs = None;
        self.writes = None; // the writer drains its queue, then ends: nothing is lost on quit
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

pub struct Streamer {
    dim: Dim,
    gen: Gen,
    workers: Workers,
    has_store: bool,
    decorated: HashSet<(i32, i32)>,
    finished: HashSet<(i32, i32)>,
    /// chunks asked of the workers and not yet back
    requested: HashSet<(i32, i32)>,
    cmin: i32,
    cmax: i32,
    /// chunks kept complete around the player
    pub radius: i32,
    seed: i64,
    /// time spent per stage (terrain wait, decorate, light), for profiling
    pub stats: [Duration; 3],
    /// the last `step` did something (inserted, decorated or finished a chunk)
    worked: bool,
}

/// At most this many chunks are out with the workers at once (nearest first).
const MAX_IN_FLIGHT: usize = 32;

fn tes_of(lv: &Level, c: (i32, i32)) -> Vec<TilePos> {
    lv.tile_entities.iter().filter(|(p, _)| (p.0 >> 4, p.2 >> 4) == c).map(|(p, t)| (*p, t.clone())).collect()
}

fn dist2(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).pow(2) + (a.1 - b.1).pow(2)
}

impl Streamer {
    /// `initial` is the radius that `world::generate` already produced (so those chunks count as done).
    pub fn new(dim: Dim, seed: i64, lv: &Level, initial: i32, radius: i32, store: Option<ChunkStore>) -> Streamer {
        let gen = match dim {
            Dim::Overworld => Gen::Over(Box::new(Structures::new(seed, lv.xz_size))),
            Dim::Nether => {
                let p = nether::NetherParams::new(seed, crate::world::INFINITE_XZ);
                Gen::Nether(Box::new(nether::NetherStructures::new(&p)))
            }
            Dim::End => Gen::End,
            Dim::Flat => Gen::Flat(Box::new(flat::FlatLevelSource::new(seed, lv.xz_size, true))),
        };
        let (cmin, cmax) = (lv.min_xz().div_euclid(16), lv.max_xz().div_euclid(16));
        let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 3));
        let workers = Workers::start(dim, seed, lv.xz_size, store.clone(), threads);
        let mut s = Streamer { dim, gen, workers, has_store: store.is_some(), decorated: HashSet::new(), finished: HashSet::new(), requested: HashSet::new(), cmin, cmax, radius: radius.max(2), seed, stats: [Duration::ZERO; 3], worked: false };
        for cx in -initial..initial {
            for cz in -initial..initial {
                if cx < initial - 1 && cz < initial - 1 {
                    s.decorated.insert((cx, cz));
                }
                if cx > -initial && cz > -initial && cx < initial - 2 && cz < initial - 2 {
                    s.finished.insert((cx, cz));
                }
            }
        }
        s
    }

    fn in_bounds(&self, c: (i32, i32)) -> bool {
        c.0 >= self.cmin && c.0 <= self.cmax && c.1 >= self.cmin && c.1 <= self.cmax
    }

    fn can_decorate(&self, c: (i32, i32)) -> bool {
        c.0 + 1 <= self.cmax && c.1 + 1 <= self.cmax
    }

    /// A chunk back from a worker goes into the level, unless the player has already moved away from it.
    fn accept(&mut self, lv: &mut Level, l: Loaded, centre: (i32, i32)) {
        self.requested.remove(&l.c);
        let far = self.radius + 3;
        if (l.c.0 - centre.0).abs() > far || (l.c.1 - centre.1).abs() > far || lv.has_chunk(l.c.0, l.c.1) {
            return;
        }
        lv.insert_chunk(l.chunk);
        for (p, te) in l.tes {
            lv.tile_entities.insert(p, te);
        }
        if l.decorated {
            self.decorated.insert(l.c);
        }
        self.worked = true;
    }

    fn drain(&mut self, lv: &mut Level, centre: (i32, i32), limit: usize) {
        for _ in 0..limit {
            match self.workers.results.try_recv() {
                Ok(l) => self.accept(lv, l, centre),
                Err(_) => break,
            }
        }
    }

    /// Wait (briefly) for one chunk from the workers.
    fn wait_one(&mut self, lv: &mut Level, centre: (i32, i32)) {
        if self.requested.is_empty() {
            return;
        }
        let t0 = Instant::now();
        match self.workers.results.recv_timeout(Duration::from_millis(200)) {
            Ok(l) => self.accept(lv, l, centre),
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {}
        }
        self.stats[0] += t0.elapsed();
    }

    fn queue_write(&self, lv: &Level, c: (i32, i32), game_time: i64) {
        if !self.has_store || !self.decorated.contains(&c) {
            return;
        }
        let (Some(ch), Some(tx)) = (lv.chunk(c.0, c.1), self.workers.writes.as_ref()) else { return };
        let job = Arc::new(WriteJob { chunk: ch.clone(), tes: tes_of(lv, c), time: game_time });
        self.workers.pending.lock().unwrap().insert(c, job.clone());
        let _ = tx.send((c, job));
    }

    fn decorate(&mut self, lv: &mut Level, c: (i32, i32)) {
        let seed = self.seed;
        let lighting = std::mem::replace(&mut lv.lighting, false);
        match &mut self.gen {
            Gen::Over(st) => mc_worldgen::post_process::post_process_chunk(lv, st, seed, c.0, c.1),
            Gen::Nether(st) => nether::post_process_chunk(lv, st, seed, c.0, c.1),
            Gen::End => end::post_process_chunk(lv, seed, c.0, c.1),
            Gen::Flat(s) => s.post_process(lv, c.0, c.1),
        }
        lv.lighting = lighting;
        self.decorated.insert(c);
    }

    fn ready_to_finish(&self, lv: &Level, c: (i32, i32)) -> bool {
        for dx in -1..=1 {
            for dz in -1..=1 {
                let n = (c.0 + dx, c.1 + dz);
                if self.in_bounds(n) && !lv.has_chunk(n.0, n.1) {
                    return false;
                }
            }
        }
        // every decoration that can change this chunk or its neighbours (heightmaps, light) must be done
        for dx in -2..=0 {
            for dz in -2..=0 {
                let n = (c.0 + dx, c.1 + dz);
                if self.in_bounds(n) && self.can_decorate(n) && !self.decorated.contains(&n) {
                    return false;
                }
            }
        }
        true
    }

    /// Advance streaming around chunk `centre` for at most `budget` (one step always runs).
    pub fn update(&mut self, lv: &mut Level, centre: (i32, i32), game_time: i64, budget: Duration) -> StreamOut {
        self.step(lv, centre, game_time, budget)
    }

    fn step(&mut self, lv: &mut Level, centre: (i32, i32), game_time: i64, budget: Duration) -> StreamOut {
        let mut out = StreamOut::default();
        self.worked = false;
        let start = Instant::now();
        // unload what is far away (the save is written by the writer thread)
        let far = self.radius + 3;
        let gone: Vec<(i32, i32)> = lv.chunk_coords().filter(|&c| (c.0 - centre.0).abs() > far || (c.1 - centre.1).abs() > far).collect();
        for c in gone {
            self.queue_write(lv, c, game_time);
            lv.tile_entities.retain(|p, _| (p.0 >> 4, p.2 >> 4) != c);
            lv.remove_chunk(c.0, c.1);
            self.decorated.remove(&c);
            self.finished.remove(&c);
            out.unloaded.push(c);
        }
        self.drain(lv, centre, 16);

        // one pass over the window: ask the workers for missing terrain (nearest first), list the main-thread work
        let r = self.radius + 2;
        let mut missing: Vec<(i32, (i32, i32))> = Vec::new();
        let mut work: Vec<(u8, i32, (i32, i32))> = Vec::new();
        for cx in centre.0 - r..=centre.0 + r {
            for cz in centre.1 - r..=centre.1 + r {
                let c = (cx, cz);
                if !self.in_bounds(c) {
                    continue;
                }
                if !lv.has_chunk(cx, cz) {
                    if !self.requested.contains(&c) {
                        missing.push((dist2(c, centre), c));
                    }
                    continue;
                }
                let d = dist2(c, centre);
                if (cx - centre.0).abs() <= self.radius && (cz - centre.1).abs() <= self.radius && !self.finished.contains(&c) && self.ready_to_finish(lv, c) {
                    work.push((0, d, c)); // light + mesh: what the player sees comes first
                } else if !self.decorated.contains(&c) && self.can_decorate(c) && [(1, 0), (0, 1), (1, 1)].iter().all(|(dx, dz)| lv.has_chunk(cx + dx, cz + dz)) {
                    work.push((1, d, c));
                }
            }
        }
        missing.sort_unstable();
        if let Some(tx) = self.workers.jobs.as_ref() {
            for (_, c) in missing {
                if self.requested.len() >= MAX_IN_FLIGHT {
                    break;
                }
                if tx.send(c).is_ok() {
                    self.requested.insert(c);
                }
            }
        }
        work.sort_unstable();

        let mut finishes = 0;
        for (i, (kind, _, c)) in work.into_iter().enumerate() {
            if i > 0 && (start.elapsed() >= budget || finishes >= 3) {
                break;
            }
            let t0 = Instant::now();
            if kind == 1 {
                if !self.decorated.contains(&c) {
                    self.decorate(lv, c);
                }
                self.stats[1] += t0.elapsed();
            } else {
                if self.finished.contains(&c) || !self.ready_to_finish(lv, c) {
                    continue;
                }
                lv.light_chunk(c.0, c.1);
                lv.light_lava(c.0, c.1);
                self.finished.insert(c);
                out.meshed.push(c);
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        let n = (c.0 + dx, c.1 + dz);
                        if n != c && self.finished.contains(&n) {
                            out.remesh.push(n);
                        }
                    }
                }
                finishes += 1;
                self.stats[2] += t0.elapsed();
            }
            self.worked = true;
        }
        out
    }

    /// Run until nothing is left to do around `centre` (loading a save, or the first frames of a world).
    pub fn pump_all(&mut self, lv: &mut Level, centre: (i32, i32), game_time: i64) -> StreamOut {
        let mut all = StreamOut::default();
        loop {
            let o = self.step(lv, centre, game_time, Duration::from_secs(3600));
            all.meshed.extend(o.meshed);
            all.remesh.extend(o.remesh);
            all.unloaded.extend(o.unloaded);
            if !self.worked {
                if self.requested.is_empty() {
                    break;
                }
                self.wait_one(lv, centre);
            }
        }
        all
    }

    /// Hand the chunks in `which` to the writer thread (autosave of edited chunks).
    pub fn save_some(&self, lv: &Level, game_time: i64, which: &HashSet<(i32, i32)>) {
        for c in which {
            self.queue_write(lv, *c, game_time);
        }
    }

    /// Hand every decorated chunk that is loaded to the writer thread (autosave / quit; dropping the
    /// streamer waits for the writer to finish).
    pub fn save_all(&self, lv: &Level, game_time: i64) {
        for c in lv.chunk_coords() {
            self.queue_write(lv, c, game_time);
        }
    }

    /// Wait until every queued write has reached the disk.
    pub fn flush(&self) {
        while !self.workers.pending.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// An empty level of the right shape for `dim` (no chunks yet).
pub fn empty_level(dim: Dim, seed: i64) -> Level {
    generate(dim, seed, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn bench_stages() {
        let mut lv = generate(Dim::Overworld, 7, 3);
        let mut s = Streamer::new(Dim::Overworld, 7, &lv, 3, 8, None);
        let t = Instant::now();
        let o = s.pump_all(&mut lv, (20, 0), 0);
        let mut m = Duration::ZERO;
        for &(x, z) in &o.meshed {
            let t = Instant::now();
            let _ = crate::mesh::mesh_chunk(&lv, x, z);
            m += t.elapsed();
        }
        eprintln!("chunks {} total {:?} terrain {:?} deco {:?} light {:?} mesh {:?}", o.meshed.len(), t.elapsed(), s.stats[0], s.stats[1], s.stats[2], m);
    }

    #[test]
    fn walking_far_streams_new_chunks_and_drops_old_ones() {
        let mut lv = generate(Dim::Flat, 1, 3);
        let mut s = Streamer::new(Dim::Flat, 1, &lv, 3, 3, None);
        assert!(lv.has_chunk(0, 0) && !lv.has_chunk(10, 0));
        let o = s.pump_all(&mut lv, (10, 0), 0);
        assert!(lv.has_chunk(10, 0) && lv.has_chunk(10, 3));
        assert!(!lv.has_chunk(-3, 0), "far chunks are unloaded");
        assert!(o.unloaded.contains(&(0, 0)));
        assert!(o.meshed.contains(&(10, 0)));
        // lit and meshable
        let m = crate::mesh::mesh_chunk(&lv, 10, 0);
        assert!(!m.opaque.indices.is_empty());
    }

    #[test]
    fn the_world_has_no_practical_edge() {
        let mut lv = generate(Dim::Flat, 1, 3);
        let mut s = Streamer::new(Dim::Flat, 1, &lv, 3, 3, None);
        // 100k chunks (1.6 million blocks) from the origin still streams
        s.pump_all(&mut lv, (100_000, -100_000), 0);
        assert!(lv.has_chunk(100_000, -100_000));
        assert!(lv.chunk_coords().all(|(x, z)| (x - 100_000).abs() <= 6 && (z + 100_000).abs() <= 6));
    }

    #[test]
    fn saved_chunks_come_back_with_player_edits() {
        let d = std::env::temp_dir().join(format!("mcrs_stream_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let store = ChunkStore::new(d.clone(), Dim::Flat);
        let mut lv = generate(Dim::Flat, 1, 3);
        let mut s = Streamer::new(Dim::Flat, 1, &lv, 3, 3, Some(store.clone()));
        lv.set_tile(2, 40, 2, 4, 0);
        s.pump_all(&mut lv, (12, 0), 0); // unloads and saves chunk (0,0)
        s.flush();
        assert!(!lv.has_chunk(0, 0));
        s.pump_all(&mut lv, (0, 0), 0); // loads it back
        assert_eq!(lv.get_tile(2, 40, 2), 4);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn chest_contents_survive_unloading() {
        use mc_level::tile_entity::{ItemStack, TileEntity};
        let d = std::env::temp_dir().join(format!("mcrs_stream_te_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let store = ChunkStore::new(d.clone(), Dim::Flat);
        let mut lv = generate(Dim::Flat, 1, 3);
        let mut s = Streamer::new(Dim::Flat, 1, &lv, 3, 3, Some(store));
        lv.set_tile(2, 40, 2, mc_world::tile_ids::CHEST, 0);
        let mut items = vec![None; 27];
        items[4] = Some(ItemStack { id: 264, count: 7, damage: 0, enchantments: vec![] });
        lv.tile_entities.insert((2, 40, 2), TileEntity::Chest { items: items.clone() });
        s.pump_all(&mut lv, (12, 0), 0);
        s.flush();
        assert!(lv.tile_entities.is_empty(), "unloaded chunks release their tile entities");
        s.pump_all(&mut lv, (0, 0), 0);
        assert_eq!(lv.tile_entities.get(&(2, 40, 2)), Some(&TileEntity::Chest { items }));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn overworld_streaming_decorates_like_the_batch_generator() {
        let mut lv = generate(Dim::Overworld, 7, 3);
        let mut s = Streamer::new(Dim::Overworld, 7, &lv, 3, 3, None);
        s.pump_all(&mut lv, (6, 0), 0);
        assert!(lv.has_chunk(6, 0));
        let top = lv.get_heightmap(6 * 16 + 4, 4);
        use mc_level::chunk::LightLayer;
        assert_eq!(lv.get_brightness(LightLayer::Sky, 6 * 16 + 4, top + 2, 4), 15);
    }
}
