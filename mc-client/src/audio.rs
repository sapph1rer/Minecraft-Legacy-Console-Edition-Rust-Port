//! Sound effects: a replacement for `SoundEngine` (Miles/miniaudio in the original).
//!
//! * `SoundBank` resolves a sound *name* (`"dig.stone"`) to a file the same way `SoundEngine::play` does:
//!   `Windows64Media/Sound/Minecraft/dig/stone.ogg` if it exists, otherwise a random one of the numbered
//!   variants `stone1.ogg … stone31.ogg` (`rand() % count + 1`, `count` = highest existing number).
//! * `Mixer` mixes decoded samples with pitch (resampling), volume (clamped to 1 like the original) and the
//!   miniaudio defaults for 3-D sources: inverse-distance attenuation (min distance 1, rolloff 1) and
//!   left/right balance from the listener orientation.
//! * `AudioEngine` ties both to a `cpal` output stream (feature `audio`, default on). Without a device (or
//!   with the feature off) everything still decodes and mixes, it just is not played.

use mc_core::random::Random;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// One sound to play (`SoundEngine::play(iSound, x, y, z, volume, pitch)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundEvent {
    /// `ConsoleSoundEngine::wchSoundNames` entry, e.g. `"dig.stone"`.
    pub name: &'static str,
    pub pos: [f64; 3],
    pub volume: f32,
    pub pitch: f32,
}

/// Decoded audio, interleaved f32.
pub struct Sample {
    pub rate: u32,
    pub channels: u16,
    pub data: Vec<f32>,
}

impl Sample {
    pub fn frames(&self) -> usize {
        self.data.len() / self.channels.max(1) as usize
    }
}

pub fn decode_ogg(bytes: &[u8]) -> Result<Sample, String> {
    use lewton::inside_ogg::OggStreamReader;
    let mut rd = OggStreamReader::new(std::io::Cursor::new(bytes)).map_err(|e| format!("{e:?}"))?;
    let rate = rd.ident_hdr.audio_sample_rate;
    let channels = rd.ident_hdr.audio_channels as u16;
    let mut data = Vec::new();
    while let Some(pck) = rd.read_dec_packet_itl().map_err(|e| format!("{e:?}"))? {
        data.extend(pck.iter().map(|&s| s as f32 / 32768.0));
    }
    Ok(Sample { rate, channels, data })
}

// ------------------------------------------------------------------------------------------------ bank

pub struct SoundBank {
    root: PathBuf,
    cache: HashMap<PathBuf, Option<Arc<Sample>>>,
    random: Random,
}

impl SoundBank {
    /// `root` = the `Windows64Media/Sound` folder (containing `Minecraft/`).
    pub fn new(root: impl Into<PathBuf>) -> SoundBank {
        SoundBank { root: root.into(), cache: HashMap::new(), random: Random::new() }
    }

    fn base(&self, name: &str) -> PathBuf {
        // ConvertSoundPathToName: dots become path separators
        let mut p = self.root.join("Minecraft");
        for part in name.split('.') {
            p.push(part);
        }
        p
    }

    fn with_ext(base: &Path, suffix: &str) -> PathBuf {
        let mut s = base.as_os_str().to_owned();
        s.push(suffix);
        PathBuf::from(s)
    }

    /// The file `SoundEngine::play` would pick for `name` (random variant included).
    pub fn resolve(&mut self, name: &str) -> Option<PathBuf> {
        let base = self.base(name);
        let exact = Self::with_ext(&base, ".ogg");
        if exact.is_file() {
            return Some(exact);
        }
        let mut count = 0;
        for i in 1..32 {
            if Self::with_ext(&base, &format!("{i}.ogg")).is_file() {
                count = i;
            }
        }
        if count == 0 {
            return None;
        }
        let chosen = self.random.next_int_bounded(count as i32) as usize + 1;
        let p = Self::with_ext(&base, &format!("{chosen}.ogg"));
        p.is_file().then_some(p)
    }

    pub fn load(&mut self, path: &Path) -> Option<Arc<Sample>> {
        if let Some(c) = self.cache.get(path) {
            return c.clone();
        }
        let s = std::fs::read(path).ok().and_then(|b| decode_ogg(&b).ok()).map(Arc::new);
        self.cache.insert(path.to_path_buf(), s.clone());
        s
    }

    pub fn sample_for(&mut self, name: &str) -> Option<Arc<Sample>> {
        let p = self.resolve(name)?;
        self.load(&p)
    }
}

// ------------------------------------------------------------------------------------------------ mixer

#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub pos: [f64; 3],
    /// unit forward vector (`vOrientFront`)
    pub front: [f64; 3],
}

impl Default for Listener {
    fn default() -> Self {
        Listener { pos: [0.0; 3], front: [0.0, 0.0, 1.0] }
    }
}

struct Voice {
    sample: Arc<Sample>,
    /// position in source frames
    pos: f64,
    pitch: f64,
    volume: f32,
    source: [f64; 3],
}

pub const MAX_VOICES: usize = 48;

pub struct Mixer {
    voices: Vec<Voice>,
    pub out_rate: u32,
    pub listener: Listener,
    /// `m_MasterEffectsVolume`
    pub master: f32,
}

/// miniaudio defaults: inverse attenuation with min distance 1, rolloff 1, max distance unbounded.
pub fn distance_gain(d: f64) -> f32 {
    distance_gain_vol(d, 1.0)
}

/// `SoundEngine`: a sound is audible within 16 blocks (x volume when louder than 1), fading linearly to silence.
pub fn distance_gain_vol(d: f64, volume: f32) -> f32 {
    let max = 16.0 * (volume as f64).max(1.0);
    (1.0 - (d - 1.0).max(0.0) / (max - 1.0)).clamp(0.0, 1.0) as f32
}

/// Left/right balance gains for a source at `rel` (relative to the listener) given the listener's front.
pub fn balance(front: [f64; 3], rel: [f64; 3]) -> (f32, f32) {
    let right = [-front[2], 0.0, front[0]];
    let rl = (right[0] * right[0] + right[2] * right[2]).sqrt();
    let d = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
    if rl < 1e-9 || d < 1e-6 {
        return (1.0, 1.0);
    }
    let p = ((rel[0] * right[0] + rel[2] * right[2]) / (rl * d)).clamp(-1.0, 1.0) as f32;
    ((1.0 - p).min(1.0), (1.0 + p).min(1.0))
}

impl Mixer {
    pub fn new(out_rate: u32) -> Mixer {
        Mixer { voices: Vec::new(), out_rate, listener: Listener::default(), master: 1.0 }
    }

    pub fn active(&self) -> usize {
        self.voices.len()
    }

    pub fn play(&mut self, sample: Arc<Sample>, ev: &SoundEvent) {
        // too far away to be heard at all
        let l = self.listener.pos;
        let rel = [ev.pos[0] - l[0], ev.pos[1] - l[1], ev.pos[2] - l[2]];
        let dist = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
        if distance_gain_vol(dist, ev.volume) <= 0.0 {
            return;
        }
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice { sample, pos: 0.0, pitch: ev.pitch.max(0.01) as f64, volume: ev.volume, source: ev.pos });
    }

    /// Mix `out.len() / 2` stereo frames into `out` (overwritten).
    pub fn render_stereo(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        let l = self.listener;
        let out_rate = self.out_rate as f64;
        for v in &mut self.voices {
            let rel = [v.source[0] - l.pos[0], v.source[1] - l.pos[1], v.source[2] - l.pos[2]];
            let dist = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
            let gain = (v.volume * self.master).min(1.0) * distance_gain_vol(dist, v.volume);
            let (gl, gr) = balance(l.front, rel);
            let step = v.sample.rate as f64 * v.pitch / out_rate;
            let ch = v.sample.channels as usize;
            let total = v.sample.frames();
            for f in 0..frames {
                let i = v.pos as usize;
                if i >= total {
                    break;
                }
                let fr = (v.pos - i as f64) as f32;
                let j = (i + 1).min(total - 1);
                let s = |frame: usize, c: usize| v.sample.data[frame * ch + c.min(ch - 1)];
                let a_l = s(i, 0) * (1.0 - fr) + s(j, 0) * fr;
                let a_r = if ch > 1 { s(i, 1) * (1.0 - fr) + s(j, 1) * fr } else { a_l };
                out[f * 2] += a_l * gain * gl;
                out[f * 2 + 1] += a_r * gain * gr;
                v.pos += step;
            }
        }
        self.voices.retain(|v| (v.pos as usize) < v.sample.frames());
        for o in out.iter_mut() {
            *o = o.clamp(-1.0, 1.0);
        }
    }
}

// ------------------------------------------------------------------------------------------------ engine

pub struct AudioEngine {
    bank: SoundBank,
    mixer: Arc<Mutex<Mixer>>,
    #[cfg(feature = "audio")]
    _stream: Option<cpal::Stream>,
    pub device_name: Option<String>,
}

impl AudioEngine {
    /// `sound_root` = `Windows64Media/Sound`.
    pub fn new(sound_root: impl Into<PathBuf>) -> AudioEngine {
        #[allow(unused_mut)]
        let mut out_rate = 44100;
        #[cfg(feature = "audio")]
        let stream;
        #[allow(unused_mut)]
        let mut device_name = None;
        let mixer;
        #[cfg(feature = "audio")]
        {
            let (m, s, rate, name) = start_stream();
            mixer = m;
            stream = s;
            out_rate = rate.unwrap_or(out_rate);
            device_name = name;
        }
        #[cfg(not(feature = "audio"))]
        {
            mixer = Arc::new(Mutex::new(Mixer::new(out_rate)));
        }
        let _ = out_rate;
        AudioEngine {
            bank: SoundBank::new(sound_root),
            mixer,
            #[cfg(feature = "audio")]
            _stream: stream,
            device_name,
        }
    }

    /// `SoundEngine::play`
    pub fn play(&mut self, ev: &SoundEvent) {
        if let Some(s) = self.bank.sample_for(ev.name) {
            if let Ok(mut m) = self.mixer.lock() {
                m.play(s, ev);
            }
        }
    }

    /// `SoundEngine::updatePlayerPosition / listener`
    pub fn set_listener(&mut self, pos: [f64; 3], front: [f64; 3]) {
        if let Ok(mut m) = self.mixer.lock() {
            m.listener = Listener { pos, front };
        }
    }

    pub fn set_effects_volume(&mut self, v: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.master = v;
        }
    }
}

#[cfg(feature = "audio")]
fn start_stream() -> (Arc<Mutex<Mixer>>, Option<cpal::Stream>, Option<u32>, Option<String>) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let fallback = || (Arc::new(Mutex::new(Mixer::new(44100))), None, None, None);
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else { return fallback() };
    let Ok(cfg) = device.default_output_config() else { return fallback() };
    let rate = cfg.sample_rate();
    let channels = cfg.channels() as usize;
    let mixer = Arc::new(Mutex::new(Mixer::new(rate)));
    let m = mixer.clone();
    let mut scratch: Vec<f32> = Vec::new();
    let err = |e: cpal::Error| eprintln!("audio stream error: {e}");
    let mut fill = move |out: &mut [f32]| {
        let frames = out.len() / channels;
        scratch.resize(frames * 2, 0.0);
        if let Ok(mut mx) = m.try_lock() {
            mx.render_stereo(&mut scratch);
        } else {
            scratch.fill(0.0);
        }
        for f in 0..frames {
            for c in 0..channels {
                out[f * channels + c] = match c {
                    0 => scratch[f * 2],
                    1 => scratch[f * 2 + 1],
                    _ => 0.0,
                };
            }
        }
    };
    let config: cpal::StreamConfig = cfg.clone().into();
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(config, move |d: &mut [f32], _: &cpal::OutputCallbackInfo| fill(d), err, None),
        _ => return fallback(),
    };
    match stream {
        Ok(s) => {
            let _ = s.play();
            (mixer, Some(s), Some(rate), Some(device.to_string()))
        }
        Err(_) => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/mnt/user-data/uploads/MinecraftConsoles-main/Minecraft.Client/Windows64Media/Sound")
    }

    fn tone(rate: u32, secs: f32) -> Arc<Sample> {
        let n = (rate as f32 * secs) as usize;
        Arc::new(Sample { rate, channels: 1, data: (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5).collect() })
    }

    #[test]
    fn distance_and_balance_follow_miniaudio_defaults() {
        assert_eq!(distance_gain(0.3), 1.0);
        assert_eq!(distance_gain(1.0), 1.0);
        assert!((distance_gain(4.0) - 0.8).abs() < 1e-6);
        assert_eq!(distance_gain(16.0), 0.0);
        assert_eq!(distance_gain(100.0), 0.0);
        // listener faces +z (south): its right-hand side is -x (west)
        let front = [0.0, 0.0, 1.0];
        let (l, r) = balance(front, [-5.0, 0.0, 0.0]);
        assert!(r > l, "source on the right should be louder on the right: {l} {r}");
        let (l, r) = balance(front, [0.0, 0.0, 5.0]);
        assert_eq!((l, r), (1.0, 1.0));
        // facing north (-z) the right-hand side is +x (east)
        let (l, r) = balance([0.0, 0.0, -1.0], [3.0, 0.0, 0.0]);
        assert!(r > l);
    }

    #[test]
    fn mixer_plays_a_voice_to_the_end_and_drops_it() {
        let mut m = Mixer::new(1000);
        m.play(tone(1000, 0.1), &SoundEvent { name: "x", pos: [0.0; 3], volume: 1.0, pitch: 1.0 });
        let mut out = vec![0.0f32; 400];
        m.render_stereo(&mut out);
        assert!(out[..200].iter().any(|v| v.abs() > 0.1));
        assert_eq!(m.active(), 0);
        assert!(out[200..].iter().all(|&v| v == 0.0));
    }

    #[test]
    fn pitch_two_plays_twice_as_fast_and_volume_is_clamped() {
        let mut m = Mixer::new(1000);
        m.play(tone(1000, 0.2), &SoundEvent { name: "x", pos: [0.0; 3], volume: 5.0, pitch: 2.0 });
        let mut out = vec![0.0f32; 400]; // 200 frames
        m.render_stereo(&mut out);
        // 200 source frames at 2x -> done after ~100 output frames
        assert_eq!(m.active(), 0);
        assert!(out[220..].iter().all(|&v| v == 0.0));
        assert!(out.iter().all(|v| v.abs() <= 0.5 + 1e-3), "volume above 1 must be clamped to 1");
    }

    #[test]
    fn distant_sound_is_quieter() {
        let ev = |x: f64| SoundEvent { name: "x", pos: [x, 0.0, 0.0], volume: 1.0, pitch: 1.0 };
        let peak = |x: f64| {
            let mut m = Mixer::new(1000);
            m.play(tone(1000, 0.1), &ev(x));
            let mut out = vec![0.0f32; 100];
            m.render_stereo(&mut out);
            out.iter().fold(0.0f32, |a, &b| a.max(b.abs()))
        };
        assert!(peak(8.0) < peak(1.0) * 0.7);
        assert_eq!(peak(40.0), 0.0);
    }

    #[test]
    fn bank_resolves_numbered_variants_and_decodes_ogg() {
        let r = root();
        if !r.join("Minecraft/dig/stone1.ogg").exists() {
            eprintln!("sound assets not staged; skipping");
            return;
        }
        let mut b = SoundBank::new(&r);
        for _ in 0..8 {
            let p = b.resolve("dig.stone").expect("variant");
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            assert!(name == "stone1.ogg" || name == "stone2.ogg", "{name}"); // only 1 and 2 are staged
        }
        assert!(b.resolve("dig.nothing").is_none());
        let s = b.sample_for("liquid.water").expect("exact file");
        assert!(s.frames() > 1000 && s.rate >= 8000);
        assert!(s.data.iter().any(|v| v.abs() > 0.01));
        let g = b.sample_for("step.grass").expect("grass");
        assert!(g.frames() > 100);
        // a real sound through the mixer, played to the listener's right at 3 blocks, comes out on the right side
        let mut m = Mixer::new(44100);
        m.listener = Listener { pos: [0.0; 3], front: [0.0, 0.0, -1.0] };
        let st = crate::soundtypes::sound_type(1);
        let s = b.sample_for(st.brk).unwrap();
        m.play(s, &SoundEvent { name: st.brk, pos: [3.0, 0.0, 0.0], volume: 1.0, pitch: 0.8 });
        let mut out = vec![0.0f32; 4096 * 2];
        m.render_stereo(&mut out);
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for f in out.chunks(2) {
            l += f[0].abs();
            r += f[1].abs();
        }
        assert!(r > 0.5 && l < r, "left {l} right {r}");
    }
}
