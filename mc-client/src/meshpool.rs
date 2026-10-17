//! Chunk meshing on worker threads: a job carries a copy of the 3×3 chunks around the target, so the world
//! itself never leaves the game thread.
use crate::mesh::{mesh_chunk, ChunkMesh};
use mc_level::level::Level;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

struct Job {
    cx: i32,
    cz: i32,
    ver: u64,
    lv: Level,
}

pub struct MeshPool {
    tx: Sender<Job>,
    rx: Receiver<(i32, i32, u64, ChunkMesh)>,
    latest: HashMap<(i32, i32), u64>,
    next: u64,
}

/// A level holding only the chunks that `mesh_chunk(lv, cx, cz)` can look at.
pub fn snapshot(lv: &Level, cx: i32, cz: i32) -> Level {
    let mut s = Level::new(lv.xz_size, lv.has_ceiling);
    s.ultra_warm = lv.ultra_warm;
    s.lighting = false;
    for dx in -1..=1 {
        for dz in -1..=1 {
            if let Some(c) = lv.chunk(cx + dx, cz + dz) {
                s.insert_chunk(c.clone());
            }
        }
    }
    s
}

impl MeshPool {
    pub fn new(threads: usize) -> MeshPool {
        let (tx, jobs) = channel::<Job>();
        let (rtx, rx) = channel();
        let jobs = Arc::new(Mutex::new(jobs));
        for _ in 0..threads.max(1) {
            let (jobs, rtx) = (jobs.clone(), rtx.clone());
            std::thread::spawn(move || loop {
                let job = match jobs.lock().map(|j| j.recv()) {
                    Ok(Ok(j)) => j,
                    _ => return,
                };
                let m = mesh_chunk(&job.lv, job.cx, job.cz);
                if rtx.send((job.cx, job.cz, job.ver, m)).is_err() {
                    return;
                }
            });
        }
        MeshPool { tx, rx, latest: HashMap::new(), next: 0 }
    }

    pub fn default_threads() -> usize {
        std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1)).clamp(1, 4)
    }

    /// Queue a (re)mesh of chunk (`cx`, `cz`); an older result for the same chunk is discarded.
    pub fn submit(&mut self, lv: &Level, cx: i32, cz: i32) {
        if !lv.has_chunk(cx, cz) {
            return;
        }
        self.next += 1;
        self.latest.insert((cx, cz), self.next);
        let _ = self.tx.send(Job { cx, cz, ver: self.next, lv: snapshot(lv, cx, cz) });
    }

    /// Finished meshes (at most `max`), newest versions only.
    pub fn poll(&mut self, max: usize) -> Vec<(i32, i32, ChunkMesh)> {
        let mut out = Vec::new();
        while out.len() < max {
            let Ok((cx, cz, ver, m)) = self.rx.try_recv() else { break };
            if self.latest.get(&(cx, cz)) == Some(&ver) {
                self.latest.remove(&(cx, cz));
                out.push((cx, cz, m));
            }
        }
        out
    }

    pub fn forget(&mut self, cx: i32, cz: i32) {
        self.latest.remove(&(cx, cz));
    }

    pub fn pending(&self) -> usize {
        self.latest.len()
    }

    /// Mesh all `coords` in parallel and wait for the result.
    pub fn mesh_all(&mut self, lv: &Level, coords: &[(i32, i32)]) -> Vec<(i32, i32, ChunkMesh)> {
        for &(cx, cz) in coords {
            self.submit(lv, cx, cz);
        }
        let mut out = Vec::new();
        while self.pending() > 0 {
            match self.rx.recv() {
                Ok((cx, cz, ver, m)) => {
                    if self.latest.get(&(cx, cz)) == Some(&ver) {
                        self.latest.remove(&(cx, cz));
                        out.push((cx, cz, m));
                    }
                }
                Err(_) => break,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{generate, Dim};

    #[test]
    fn threaded_meshes_equal_direct_meshes() {
        let lv = generate(Dim::Overworld, 5, 3);
        let mut p = MeshPool::new(2);
        let coords = [(0, 0), (1, -1), (-1, 1)];
        let got = p.mesh_all(&lv, &coords);
        assert_eq!(got.len(), 3);
        for (cx, cz, m) in got {
            let d = mesh_chunk(&lv, cx, cz);
            assert_eq!(m.opaque.indices.len(), d.opaque.indices.len(), "{cx},{cz}");
            assert_eq!(m.translucent.indices.len(), d.translucent.indices.len());
        }
    }

    #[test]
    fn a_newer_submit_supersedes_an_older_one() {
        let mut lv = generate(Dim::Flat, 1, 3);
        let mut p = MeshPool::new(1);
        p.submit(&lv, 0, 0);
        lv.set_tile(2, 10, 2, 1, 0);
        p.submit(&lv, 0, 0);
        let got = p.mesh_all(&lv, &[]);
        assert_eq!(got.len(), 1);
    }
}
