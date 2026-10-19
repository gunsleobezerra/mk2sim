//! Transport clock: BPM, quantization grid and loop synchronization.

use std::time::Instant;

use crate::state::{Quantize, SyncMode};

pub struct Clock {
    pub sample_rate: u32,
    pub bpm: f32,
    /// Monotonic frame counter since the engine started.
    pub global: u64,
    /// Frame position inside the master loop.
    pub position: u64,
    /// Master loop length in frames (0 = no loop yet).
    pub loop_len: u64,
}

impl Clock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            bpm: 120.0,
            global: 0,
            position: 0,
            loop_len: 0,
        }
    }

    pub fn configure(&mut self, sample_rate: u32, bpm: f32) {
        self.sample_rate = sample_rate.max(8_000);
        self.bpm = bpm.clamp(20.0, 300.0);
    }

    pub fn set_loop(&mut self, loop_len: u64) {
        self.loop_len = loop_len;
        if self.loop_len == 0 {
            self.position = 0;
        } else {
            self.position %= self.loop_len;
        }
    }

    #[inline]
    pub fn tick(&mut self) {
        self.global = self.global.wrapping_add(1);
        if self.loop_len > 0 {
            self.position = (self.position + 1) % self.loop_len;
        }
    }

    /// Frames remaining until the next allowed state change.
    /// `0` means "apply immediately".
    pub fn next_boundary(&self, quantize: Quantize, sync: SyncMode) -> u64 {
        match sync {
            SyncMode::Immediate => 0,
            SyncMode::LoopLength => {
                if self.loop_len == 0 {
                    0
                } else {
                    let remaining = self.loop_len - self.position;
                    if remaining == self.loop_len {
                        0
                    } else {
                        remaining
                    }
                }
            }
            SyncMode::Measure => {
                let q = if quantize == Quantize::Off {
                    Quantize::Bar
                } else {
                    quantize
                };
                let step = q.step_frames(self.sample_rate, self.bpm);
                if step == 0 {
                    0
                } else {
                    let rem = self.global % step;
                    if rem == 0 {
                        0
                    } else {
                        step - rem
                    }
                }
            }
        }
    }

    pub fn progress(&self) -> f32 {
        if self.loop_len == 0 {
            0.0
        } else {
            self.position as f32 / self.loop_len as f32
        }
    }

    pub fn loop_len_ms(&self) -> f32 {
        if self.loop_len == 0 {
            0.0
        } else {
            self.loop_len as f32 / self.sample_rate as f32 * 1_000.0
        }
    }

    pub fn frames_to_ms(frames: u64, sample_rate: u32) -> f32 {
        frames as f32 / sample_rate as f32 * 1_000.0
    }
}

/// Estimates BPM from the length of the first recorded loop, assuming the loop
/// is exactly one 4/4 bar (RC-505 "Loop-to-Tempo" behavior).
pub fn bpm_from_loop(loop_frames: u64, sample_rate: u32) -> Option<f32> {
    if loop_frames < (sample_rate as u64 / 4) {
        return None;
    }
    let seconds = loop_frames as f32 / sample_rate as f32;
    let bpm = 240.0 / seconds;
    if (20.0..=300.0).contains(&bpm) {
        Some(bpm)
    } else {
        None
    }
}

/// Simple tap-tempo accumulator.
#[derive(Default)]
pub struct TapTempo {
    taps: Vec<Instant>,
}

impl TapTempo {
    pub fn tap(&mut self) -> Option<f32> {
        let now = Instant::now();
        if let Some(last) = self.taps.last() {
            if now.duration_since(*last).as_secs_f32() > 2.0 {
                self.taps.clear();
            }
        }
        self.taps.push(now);
        if self.taps.len() > 5 {
            self.taps.remove(0);
        }
        if self.taps.len() < 2 {
            return None;
        }
        let first = self.taps.first()?;
        let last = self.taps.last()?;
        let seconds = last.duration_since(*first).as_secs_f32();
        let intervals = (self.taps.len() - 1) as f32;
        if seconds <= 0.0 {
            return None;
        }
        Some((60.0 / (seconds / intervals)).clamp(20.0, 300.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_wraps_position() {
        let mut clock = Clock::new(48_000);
        clock.set_loop(100);
        for _ in 0..250 {
            clock.tick();
        }
        assert_eq!(clock.global, 250);
        assert_eq!(clock.position, 50);
    }

    #[test]
    fn immediate_sync_has_no_wait() {
        let clock = Clock::new(48_000);
        assert_eq!(clock.next_boundary(Quantize::Bar, SyncMode::Immediate), 0);
    }

    #[test]
    fn measure_boundary_waits_for_grid() {
        let mut clock = Clock::new(48_000);
        clock.configure(48_000, 120.0);
        clock.global = 1;
        let wait = clock.next_boundary(Quantize::Bar, SyncMode::Measure);
        assert_eq!(wait, 95_999);
        clock.global = 0;
        assert_eq!(clock.next_boundary(Quantize::Bar, SyncMode::Measure), 0);
    }

    #[test]
    fn loop_length_boundary_is_remaining_frames() {
        let mut clock = Clock::new(48_000);
        clock.set_loop(1_000);
        clock.position = 400;
        assert_eq!(
            clock.next_boundary(Quantize::Off, SyncMode::LoopLength),
            600
        );
    }

    #[test]
    fn loop_to_tempo_matches_one_bar() {
        // 2 s loop at 48 kHz = 1 bar at 120 BPM.
        let bpm = bpm_from_loop(96_000, 48_000).unwrap();
        assert!((bpm - 120.0).abs() < 0.01);
        assert!(bpm_from_loop(10, 48_000).is_none());
    }
}
