//! Shared, lock-free state exchanged between the UI thread and the audio thread.
//!
//! Every parameter that the audio thread needs is stored in an atomic. The UI
//! writes, the audio thread reads with `Relaxed` ordering (audio parameters do
//! not require cross-variable synchronization). State-machine transitions
//! (record / play / dub / stop) travel through a lock-free channel so they are
//! applied exactly once and in order.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

pub const MAX_TRACKS: usize = 5;
pub const MAX_LOOP_SECONDS: f32 = 20.0;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantize {
    Off = 0,
    Sixteenth = 1,
    Eighth = 2,
    Quarter = 3,
    Bar = 4,
    TwoBars = 5,
}

impl Quantize {
    pub const ALL: [Quantize; 6] = [
        Quantize::Off,
        Quantize::Sixteenth,
        Quantize::Eighth,
        Quantize::Quarter,
        Quantize::Bar,
        Quantize::TwoBars,
    ];

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Quantize::Sixteenth,
            2 => Quantize::Eighth,
            3 => Quantize::Quarter,
            4 => Quantize::Bar,
            5 => Quantize::TwoBars,
            _ => Quantize::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Quantize::Off => "Off",
            Quantize::Sixteenth => "1/16",
            Quantize::Eighth => "1/8",
            Quantize::Quarter => "1/4",
            Quantize::Bar => "1 Bar",
            Quantize::TwoBars => "2 Bars",
        }
    }

    /// Frames between two grid boundaries. `Off` returns 0 (no grid).
    pub fn step_frames(self, sample_rate: u32, bpm: f32) -> u64 {
        let sr = sample_rate as f64;
        let beat = 60.0 / bpm.max(20.0) as f64 * sr; // one quarter note
        let frames = match self {
            Quantize::Off => return 0,
            Quantize::Sixteenth => beat / 4.0,
            Quantize::Eighth => beat / 2.0,
            Quantize::Quarter => beat,
            Quantize::Bar => beat * 4.0,
            Quantize::TwoBars => beat * 8.0,
        };
        frames.round().max(1.0) as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Immediate = 0,
    Measure = 1,
    LoopLength = 2,
}

impl SyncMode {
    pub const ALL: [SyncMode; 3] = [SyncMode::Immediate, SyncMode::Measure, SyncMode::LoopLength];

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => SyncMode::Measure,
            2 => SyncMode::LoopLength,
            _ => SyncMode::Immediate,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SyncMode::Immediate => "Immediate",
            SyncMode::Measure => "Measure",
            SyncMode::LoopLength => "Loop Length",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DubMode {
    Overdub = 0,
    Replace1 = 1,
    Replace2 = 2,
}

impl DubMode {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => DubMode::Replace1,
            2 => DubMode::Replace2,
            _ => DubMode::Overdub,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DubMode::Overdub => "Overdub",
            DubMode::Replace1 => "Replace1",
            DubMode::Replace2 => "Replace2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackState {
    Stopped = 0,
    Recording = 1,
    Playing = 2,
    Overdubbing = 3,
}

impl TrackState {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => TrackState::Recording,
            2 => TrackState::Playing,
            3 => TrackState::Overdubbing,
            _ => TrackState::Stopped,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrackState::Stopped => "Stopped",
            TrackState::Recording => "Recording",
            TrackState::Playing => "Playing",
            TrackState::Overdubbing => "Overdubbing",
        }
    }
}

/// Commands applied by the audio thread at a quantization boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackCommand {
    Play,
    Record,
    Dub,
    Stop,
    Clear,
    Undo,
    Redo,
    ToggleReverse,
    ToggleOneShot,
    SetDubMode(DubMode),
}

impl TrackCommand {
    pub fn parse(action: &str) -> Option<Self> {
        match action {
            "play" => Some(TrackCommand::Play),
            "record" | "rec" => Some(TrackCommand::Record),
            "dub" => Some(TrackCommand::Dub),
            "stop" => Some(TrackCommand::Stop),
            "clear" => Some(TrackCommand::Clear),
            "undo" => Some(TrackCommand::Undo),
            "redo" => Some(TrackCommand::Redo),
            "reverse" => Some(TrackCommand::ToggleReverse),
            "oneshot" => Some(TrackCommand::ToggleOneShot),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Atomics helpers
// ---------------------------------------------------------------------------

#[inline]
pub fn f32_to_bits(value: f32) -> u32 {
    value.to_bits()
}

#[inline]
pub fn bits_to_f32(value: u32) -> f32 {
    f32::from_bits(value)
}

#[inline]
fn load_f32(cell: &AtomicU32) -> f32 {
    bits_to_f32(cell.load(Ordering::Relaxed))
}

#[inline]
fn store_f32(cell: &AtomicU32, value: f32) {
    cell.store(f32_to_bits(value), Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Per-track state
// ---------------------------------------------------------------------------

pub struct TrackShared {
    pub id: usize,
    pub volume: AtomicU32,
    pub pan: AtomicU32,
    pub mute: AtomicBool,
    pub reverse: AtomicBool,
    pub one_shot: AtomicBool,
    pub dub_mode: AtomicU8,
    pub fx_a: AtomicU8,
    pub fx_b: AtomicU8,
    pub fx_level: AtomicU32,
    pub eq_low: AtomicU32,
    pub eq_mid: AtomicU32,
    pub eq_high: AtomicU32,
    pub comp: AtomicBool,
    pub ns: AtomicBool,
    pub state: AtomicU8,
    pub loop_len: AtomicU64,
    pub play_pos: AtomicU64,
    pub peak: AtomicU32,
    pub rec_ms: AtomicU32,
}

impl TrackShared {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            volume: AtomicU32::new(f32_to_bits(0.85)),
            pan: AtomicU32::new(f32_to_bits(0.0)),
            mute: AtomicBool::new(false),
            reverse: AtomicBool::new(false),
            one_shot: AtomicBool::new(false),
            dub_mode: AtomicU8::new(DubMode::Overdub as u8),
            fx_a: AtomicU8::new(0),
            fx_b: AtomicU8::new(0),
            fx_level: AtomicU32::new(f32_to_bits(1.0)),
            eq_low: AtomicU32::new(f32_to_bits(0.0)),
            eq_mid: AtomicU32::new(f32_to_bits(0.0)),
            eq_high: AtomicU32::new(f32_to_bits(0.0)),
            comp: AtomicBool::new(false),
            ns: AtomicBool::new(false),
            state: AtomicU8::new(TrackState::Stopped as u8),
            loop_len: AtomicU64::new(0),
            play_pos: AtomicU64::new(0),
            peak: AtomicU32::new(0),
            rec_ms: AtomicU32::new(0),
        }
    }

    pub fn snapshot(&self) -> TrackBlockParams {
        TrackBlockParams {
            volume: load_f32(&self.volume),
            pan: load_f32(&self.pan),
            mute: self.mute.load(Ordering::Relaxed),
            reverse: self.reverse.load(Ordering::Relaxed),
            one_shot: self.one_shot.load(Ordering::Relaxed),
            dub_mode: DubMode::from_u8(self.dub_mode.load(Ordering::Relaxed)),
            fx_a: self.fx_a.load(Ordering::Relaxed),
            fx_b: self.fx_b.load(Ordering::Relaxed),
            fx_level: load_f32(&self.fx_level),
            eq_low: load_f32(&self.eq_low),
            eq_mid: load_f32(&self.eq_mid),
            eq_high: load_f32(&self.eq_high),
            comp: self.comp.load(Ordering::Relaxed),
            ns: self.ns.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrackBlockParams {
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub reverse: bool,
    pub one_shot: bool,
    pub dub_mode: DubMode,
    pub fx_a: u8,
    pub fx_b: u8,
    pub fx_level: f32,
    pub eq_low: f32,
    pub eq_mid: f32,
    pub eq_high: f32,
    pub comp: bool,
    pub ns: bool,
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

pub struct SharedState {
    pub sample_rate: AtomicU32,
    pub bpm: AtomicU32, // BPM * 100
    pub quantize: AtomicU8,
    pub sync_mode: AtomicU8,
    pub loop_len: AtomicU64,
    pub position: AtomicU64,
    pub input_fx_a: AtomicU8,
    pub input_fx_b: AtomicU8,
    pub input_fx_level: AtomicU32,
    pub input_peak: AtomicU32,
    pub cpu: AtomicU32,
    pub input_mode: AtomicU8, // 0 = live input, 1 = demo tone
    pub engine_online: AtomicBool,
    pub rhythm_on: AtomicBool,
    pub rhythm_genre: AtomicU8,
    pub rhythm_pattern: AtomicU8,
    pub rhythm_beat: AtomicU8,
    pub rhythm_volume: AtomicU32,
    pub midi_activity: AtomicU64,
    pub midi_clock: AtomicU64,
    pub memory_name: RwLock<String>,
    pub backend: RwLock<String>,
    pub device: RwLock<String>,
    pub tracks: [TrackShared; MAX_TRACKS],
}

impl SharedState {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: AtomicU32::new(sample_rate),
            bpm: AtomicU32::new(f32_to_bits(120.0)),
            quantize: AtomicU8::new(Quantize::Quarter as u8),
            sync_mode: AtomicU8::new(SyncMode::Measure as u8),
            loop_len: AtomicU64::new(0),
            position: AtomicU64::new(0),
            input_fx_a: AtomicU8::new(0),
            input_fx_b: AtomicU8::new(0),
            input_fx_level: AtomicU32::new(f32_to_bits(1.0)),
            input_peak: AtomicU32::new(0),
            cpu: AtomicU32::new(0),
            input_mode: AtomicU8::new(0),
            engine_online: AtomicBool::new(false),
            rhythm_on: AtomicBool::new(false),
            rhythm_genre: AtomicU8::new(1),
            rhythm_pattern: AtomicU8::new(0),
            rhythm_beat: AtomicU8::new(1),
            rhythm_volume: AtomicU32::new(f32_to_bits(0.65)),
            midi_activity: AtomicU64::new(0),
            midi_clock: AtomicU64::new(0),
            memory_name: RwLock::new("MEMORY 01".to_string()),
            backend: RwLock::new("starting".to_string()),
            device: RwLock::new("none".to_string()),
            tracks: std::array::from_fn(TrackShared::new),
        }
    }

    #[inline]
    pub fn bpm_value(&self) -> f32 {
        load_f32(&self.bpm)
    }

    #[inline]
    pub fn sample_rate_value(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed).max(8_000)
    }

    #[inline]
    pub fn quantize_value(&self) -> Quantize {
        Quantize::from_u8(self.quantize.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn sync_mode_value(&self) -> SyncMode {
        SyncMode::from_u8(self.sync_mode.load(Ordering::Relaxed))
    }

    pub fn set_bpm(&self, bpm: f32) {
        store_f32(&self.bpm, bpm.clamp(20.0, 300.0));
    }

    pub fn set_memory_name(&self, name: &str) {
        let mut guard = self.memory_name.write();
        *guard = name.chars().take(16).collect();
    }
}

/// Handle shared by the Tauri state and the audio thread.
pub type Shared = Arc<SharedState>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_step_is_monotonic() {
        let sr = 48_000;
        let bpm = 120.0;
        assert_eq!(Quantize::Off.step_frames(sr, bpm), 0);
        assert!(Quantize::Sixteenth.step_frames(sr, bpm) < Quantize::Eighth.step_frames(sr, bpm));
        assert!(Quantize::Eighth.step_frames(sr, bpm) < Quantize::Quarter.step_frames(sr, bpm));
        assert_eq!(Quantize::Bar.step_frames(sr, bpm), 96_000);
    }

    #[test]
    fn enums_round_trip() {
        for q in Quantize::ALL {
            assert_eq!(Quantize::from_u8(q as u8), q);
        }
        for s in SyncMode::ALL {
            assert_eq!(SyncMode::from_u8(s as u8), s);
        }
        assert_eq!(DubMode::from_u8(DubMode::Replace2 as u8), DubMode::Replace2);
    }

    #[test]
    fn bpm_is_clamped() {
        let shared = SharedState::new(48_000);
        shared.set_bpm(9_000.0);
        assert_eq!(shared.bpm_value(), 300.0);
        shared.set_bpm(1.0);
        assert_eq!(shared.bpm_value(), 20.0);
    }
}
