//! Multi-buffer looper track (RC-505 MkII "track").
//!
//! Each track owns one fixed-capacity stereo buffer. Recording fills it, playback
//! loops it, and overdub sums new input on top. Undo/redo of an overdub pass is
//! implemented by storing the captured delta of the last pass, which avoids
//! copying the whole loop.

use crate::audio::clock::Clock;
use crate::dsp::effects::{Dynamics, EffectKind, EffectSlot, ThreeBandEq};
use crate::state::{DubMode, Quantize, SyncMode, TrackBlockParams, TrackCommand, TrackShared, TrackState};

const INPUT_THRESHOLD: f32 = 0.001;

pub struct Track {
    pub id: usize,
    pub state: TrackState,
    pub loop_len: usize,
    pub play_pos: usize,
    pub rec_pos: usize,
    pub peak: f32,
    buffer: Vec<[f32; 2]>,
    max_frames: usize,
    undo_layer: Option<Vec<[f32; 2]>>,
    redo_layer: Option<Vec<[f32; 2]>>,
    capture: Option<Vec<[f32; 2]>>,
    replace2_cleared: bool,
    fx_a: EffectSlot,
    fx_b: EffectSlot,
    eq: ThreeBandEq,
    dynamics: Dynamics,
    pending: Option<(TrackCommand, u64)>,
    finalized_len: Option<usize>,
    cleared_master: bool,
}

impl Track {
    pub fn new(id: usize, sample_rate: u32, max_seconds: f32) -> Self {
        let max_frames = (sample_rate as f32 * max_seconds) as usize;
        Self {
            id,
            state: TrackState::Stopped,
            loop_len: 0,
            play_pos: 0,
            rec_pos: 0,
            peak: 0.0,
            buffer: vec![[0.0; 2]; max_frames],
            max_frames,
            undo_layer: None,
            redo_layer: None,
            capture: None,
            replace2_cleared: false,
            fx_a: EffectSlot::new(EffectKind::None),
            fx_b: EffectSlot::new(EffectKind::None),
            eq: ThreeBandEq::new(),
            dynamics: Dynamics::new(),
            pending: None,
            finalized_len: None,
            cleared_master: false,
        }
    }

    pub fn has_loop(&self) -> bool {
        self.loop_len > 0
    }

    pub fn fill(&self) -> f32 {
        if self.loop_len == 0 {
            self.rec_pos as f32 / self.max_frames as f32
        } else {
            self.loop_len as f32 / self.max_frames as f32
        }
    }

    pub fn take_finalized(&mut self) -> Option<usize> {
        self.finalized_len.take()
    }

    pub fn take_cleared(&mut self) -> bool {
        let cleared = self.cleared_master;
        self.cleared_master = false;
        cleared
    }

    pub fn pending(&self) -> Option<(TrackCommand, u64)> {
        self.pending
    }

    /// Schedules a command according to the current quantize/sync settings.
    pub fn request(
        &mut self,
        command: TrackCommand,
        clock: &Clock,
        quantize: Quantize,
        sync: SyncMode,
        shared: &TrackShared,
    ) {
        let boundary = clock.next_boundary(quantize, sync);
        if boundary == 0 {
            self.apply(command, clock, shared);
        } else {
            self.pending = Some((command, clock.global + boundary));
        }
    }

    /// Applies a pending command whose boundary has been reached.
    pub fn tick(&mut self, clock: &Clock, shared: &TrackShared) {
        if let Some((command, at)) = self.pending {
            if clock.global >= at {
                self.pending = None;
                self.apply(command, clock, shared);
            }
        }
    }

    fn apply(&mut self, command: TrackCommand, clock: &Clock, shared: &TrackShared) {
        match command {
            TrackCommand::Play => match self.state {
                TrackState::Recording => self.finalize_recording(),
                TrackState::Stopped => {
                    if self.loop_len > 0 {
                        self.play_pos = 0;
                        self.state = TrackState::Playing;
                    }
                }
                _ => {}
            },
            TrackCommand::Record => {
                if self.state == TrackState::Recording {
                    self.finalize_recording();
                    return;
                }
                self.commit_overdub();
                self.buffer.iter_mut().for_each(|s| *s = [0.0; 2]);
                self.loop_len = clock.loop_len as usize;
                self.rec_pos = 0;
                self.play_pos = 0;
                self.undo_layer = None;
                self.redo_layer = None;
                self.capture = None;
                self.replace2_cleared = false;
                self.state = TrackState::Recording;
            }
            TrackCommand::Dub => {
                if self.loop_len == 0 {
                    self.apply(TrackCommand::Record, clock, shared);
                    return;
                }
                if self.state == TrackState::Recording {
                    self.finalize_recording();
                }
                self.play_pos = 0;
                self.replace2_cleared = false;
                self.capture = if shared.dub_mode.load(std::sync::atomic::Ordering::Relaxed)
                    == DubMode::Overdub as u8
                {
                    Some(vec![[0.0; 2]; self.loop_len])
                } else {
                    self.undo_layer = None;
                    self.redo_layer = None;
                    None
                };
                self.state = TrackState::Overdubbing;
            }
            TrackCommand::Stop => {
                if self.state == TrackState::Overdubbing {
                    self.commit_overdub();
                }
                self.state = TrackState::Stopped;
                self.play_pos = 0;
            }
            TrackCommand::Clear => {
                self.buffer.iter_mut().for_each(|s| *s = [0.0; 2]);
                self.loop_len = 0;
                self.play_pos = 0;
                self.rec_pos = 0;
                self.undo_layer = None;
                self.redo_layer = None;
                self.capture = None;
                self.state = TrackState::Stopped;
                self.cleared_master = true;
            }
            TrackCommand::Undo => {
                if let Some(layer) = self.undo_layer.take() {
                    for (dst, delta) in self.buffer.iter_mut().zip(layer.iter()) {
                        dst[0] -= delta[0];
                        dst[1] -= delta[1];
                    }
                    self.redo_layer = Some(layer);
                } else if self.loop_len > 0 {
                    // Undo of the initial recording: clear the loop.
                    self.buffer.iter_mut().for_each(|s| *s = [0.0; 2]);
                    self.loop_len = 0;
                    self.state = TrackState::Stopped;
                    self.play_pos = 0;
                }
            }
            TrackCommand::Redo => {
                if let Some(layer) = self.redo_layer.take() {
                    for (dst, delta) in self.buffer.iter_mut().zip(layer.iter()) {
                        dst[0] += delta[0];
                        dst[1] += delta[1];
                    }
                    self.undo_layer = Some(layer);
                }
            }
            TrackCommand::ToggleReverse => {
                let current = shared.reverse.load(std::sync::atomic::Ordering::Relaxed);
                shared
                    .reverse
                    .store(!current, std::sync::atomic::Ordering::Relaxed);
            }
            TrackCommand::ToggleOneShot => {
                let current = shared.one_shot.load(std::sync::atomic::Ordering::Relaxed);
                shared
                    .one_shot
                    .store(!current, std::sync::atomic::Ordering::Relaxed);
            }
            TrackCommand::SetDubMode(mode) => {
                shared
                    .dub_mode
                    .store(mode as u8, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    fn finalize_recording(&mut self) {
        let len = if self.loop_len > 0 {
            self.loop_len
        } else {
            self.rec_pos
        };
        if len == 0 {
            self.state = TrackState::Stopped;
            return;
        }
        self.loop_len = len;
        self.play_pos = 0;
        self.state = TrackState::Playing;
        self.finalized_len = Some(len);
    }

    fn commit_overdub(&mut self) {
        if let Some(capture) = self.capture.take() {
            if capture.iter().any(|s| s[0] != 0.0 || s[1] != 0.0) {
                self.undo_layer = Some(capture);
                self.redo_layer = None;
            }
        }
    }

    /// Renders one frame and returns the post-FX, post-fader stereo output.
    pub fn process(
        &mut self,
        input: [f32; 2],
        params: &TrackBlockParams,
        clock: &Clock,
    ) -> [f32; 2] {
        let sr = clock.sample_rate as f32;
        let mut out = [0.0f32; 2];

        match self.state {
            TrackState::Recording => {
                if self.rec_pos < self.max_frames
                    && (self.loop_len == 0 || self.rec_pos < self.loop_len)
                {
                    self.buffer[self.rec_pos] = input;
                    self.rec_pos += 1;
                }
                let done = if self.loop_len > 0 {
                    self.rec_pos >= self.loop_len
                } else {
                    self.rec_pos >= self.max_frames
                };
                if done {
                    self.finalize_recording();
                }
                out = input;
            }
            TrackState::Playing | TrackState::Overdubbing => {
                if self.loop_len == 0 {
                    self.state = TrackState::Stopped;
                } else {
                    let read = if params.reverse {
                        self.loop_len - 1 - self.play_pos
                    } else {
                        self.play_pos
                    };
                    if self.state == TrackState::Overdubbing {
                        match params.dub_mode {
                            DubMode::Overdub => {
                                self.buffer[read][0] += input[0];
                                self.buffer[read][1] += input[1];
                                if let Some(capture) = self.capture.as_mut() {
                                    capture[self.play_pos][0] += input[0];
                                    capture[self.play_pos][1] += input[1];
                                }
                            }
                            DubMode::Replace1 => {
                                if input[0].abs() > INPUT_THRESHOLD
                                    || input[1].abs() > INPUT_THRESHOLD
                                {
                                    self.buffer[read] = input;
                                }
                            }
                            DubMode::Replace2 => {
                                if !self.replace2_cleared {
                                    self.buffer.iter_mut().for_each(|s| *s = [0.0; 2]);
                                    self.replace2_cleared = true;
                                }
                                self.buffer[read] = input;
                            }
                        }
                    }
                    out = self.buffer[read];
                    self.play_pos += 1;
                    if self.play_pos >= self.loop_len {
                        self.play_pos = 0;
                        if params.one_shot {
                            self.state = TrackState::Stopped;
                        }
                    }
                }
            }
            TrackState::Stopped => {}
        }

        let dry = out;
        self.fx_a.set_kind(EffectKind::from_u8(params.fx_a));
        self.fx_b.set_kind(EffectKind::from_u8(params.fx_b));
        let (mut l, mut r) = self.fx_a.process(out[0], out[1], sr);
        (l, r) = self.fx_b.process(l, r, sr);
        (l, r) = self.eq.process(
            l,
            r,
            sr,
            params.eq_low,
            params.eq_mid,
            params.eq_high,
        );
        (l, r) = self.dynamics.process(l, r, sr, params.comp, params.ns);

        let level = params.fx_level.clamp(0.0, 1.0);
        l = dry[0] * (1.0 - level) + l * level;
        r = dry[1] * (1.0 - level) + r * level;

        // Linear pan law: center is unity on both channels, edges attenuate the
        // opposite side only. This matches the RC-505 headroom behaviour and keeps
        // the dry signal at 0 dB when the pan knob is centered.
        let pan = params.pan.clamp(-1.0, 1.0);
        let gain_l = 1.0 - pan.max(0.0);
        let gain_r = 1.0 + pan.min(0.0);
        let volume = params.volume.clamp(0.0, 1.0);
        let mute = if params.mute { 0.0 } else { 1.0 };

        let out = [l * gain_l * volume * mute, r * gain_r * volume * mute];
        let frame_peak = out[0].abs().max(out[1].abs());
        self.peak = (self.peak * 0.999).max(frame_peak);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{SharedState, MAX_TRACKS};

    fn params() -> TrackBlockParams {
        TrackBlockParams {
            volume: 1.0,
            pan: 0.0,
            mute: false,
            reverse: false,
            one_shot: false,
            dub_mode: DubMode::Overdub,
            fx_a: 0,
            fx_b: 0,
            fx_level: 1.0,
            eq_low: 0.0,
            eq_mid: 0.0,
            eq_high: 0.0,
            comp: false,
            ns: false,
        }
    }

    fn rig() -> (SharedState, Clock, Track) {
        let sr = 48_000;
        let shared = SharedState::new(sr);
        let mut clock = Clock::new(sr);
        clock.configure(sr, 120.0);
        let track = Track::new(0, sr, 2.0);
        (shared, clock, track)
    }

    fn request(track: &mut Track, shared: &SharedState, clock: &Clock, command: TrackCommand) {
        track.request(
            command,
            clock,
            Quantize::Off,
            SyncMode::Immediate,
            &shared.tracks[0],
        );
    }

    fn record_constant(track: &mut Track, shared: &SharedState, clock: &mut Clock, frames: usize) {
        request(track, shared, clock, TrackCommand::Record);
        for _ in 0..frames {
            track.process([0.5, 0.5], &params(), clock);
            clock.tick();
        }
        request(track, shared, clock, TrackCommand::Play);
    }

    #[test]
    fn recording_sets_loop_length_and_plays_back() {
        let (shared, mut clock, mut track) = rig();
        record_constant(&mut track, &shared, &mut clock, 1_000);
        assert_eq!(track.loop_len, 1_000);
        assert_eq!(track.state, TrackState::Playing);
        assert_eq!(track.take_finalized(), Some(1_000));

        let out = track.process([0.0, 0.0], &params(), &clock);
        assert!((out[0] - 0.5).abs() < 0.01, "playback should return recorded audio");
    }

    #[test]
    fn overdub_adds_and_undo_subtracts() {
        let (shared, mut clock, mut track) = rig();
        record_constant(&mut track, &shared, &mut clock, 500);
        request(&mut track, &shared, &clock, TrackCommand::Dub);
        assert_eq!(track.state, TrackState::Overdubbing);

        for _ in 0..500 {
            track.process([0.25, 0.25], &params(), &clock);
            clock.tick();
        }
        request(&mut track, &shared, &clock, TrackCommand::Stop);

        let out = track.process([0.0, 0.0], &params(), &clock);
        assert!(out[0].abs() < 1e-6, "stopped track is silent");

        request(&mut track, &shared, &clock, TrackCommand::Play);
        let out = track.process([0.0, 0.0], &params(), &clock);
        assert!((out[0] - 0.75).abs() < 0.02, "overdub should sum input");

        request(&mut track, &shared, &clock, TrackCommand::Undo);
        let out = track.process([0.0, 0.0], &params(), &clock);
        assert!((out[0] - 0.5).abs() < 0.02, "undo should remove the overdub layer");

        request(&mut track, &shared, &clock, TrackCommand::Redo);
        let out = track.process([0.0, 0.0], &params(), &clock);
        assert!((out[0] - 0.75).abs() < 0.02, "redo should restore the overdub layer");
    }

    #[test]
    fn reverse_plays_buffer_backwards() {
        let (shared, mut clock, mut track) = rig();
        request(&mut track, &shared, &clock, TrackCommand::Record);
        for i in 0..100 {
            let value = i as f32 / 100.0;
            track.process([value, value], &params(), &clock);
            clock.tick();
        }
        request(&mut track, &shared, &clock, TrackCommand::Play);
        let mut p = params();
        p.reverse = true;
        let first = track.process([0.0, 0.0], &p, &clock);
        assert!(
            (first[0] - 0.99).abs() < 0.02,
            "reverse should start at the end of the buffer"
        );
    }

    #[test]
    fn one_shot_stops_after_a_pass() {
        let (shared, mut clock, mut track) = rig();
        record_constant(&mut track, &shared, &mut clock, 32);
        let mut p = params();
        p.one_shot = true;
        for _ in 0..40 {
            track.process([0.0, 0.0], &p, &clock);
        }
        assert_eq!(track.state, TrackState::Stopped);
    }

    #[test]
    fn quantized_request_is_deferred() {
        let (shared, mut clock, mut track) = rig();
        clock.set_loop(1_000);
        clock.position = 250;
        clock.global = 250;
        track.request(
            TrackCommand::Record,
            &clock,
            Quantize::Quarter,
            SyncMode::LoopLength,
            &shared.tracks[0],
        );
        assert!(track.pending().is_some());
        assert_eq!(track.state, TrackState::Stopped);
        for _ in 0..750 {
            clock.tick();
            track.tick(&clock, &shared.tracks[0]);
        }
        assert_eq!(track.state, TrackState::Recording);
    }

    #[test]
    fn replace1_only_overwrites_when_input_present() {
        let (shared, mut clock, mut track) = rig();
        record_constant(&mut track, &shared, &mut clock, 100);
        let mut p = params();
        p.dub_mode = DubMode::Replace1;
        request(&mut track, &shared, &clock, TrackCommand::Dub);
        for i in 0..100 {
            let input = if i % 2 == 0 { [0.9, 0.9] } else { [0.0, 0.0] };
            track.process(input, &p, &clock);
            clock.tick();
        }
        request(&mut track, &shared, &clock, TrackCommand::Stop);
        request(&mut track, &shared, &clock, TrackCommand::Play);
        let a = track.process([0.0, 0.0], &p, &clock);
        let b = track.process([0.0, 0.0], &p, &clock);
        assert!((a[0] - 0.9).abs() < 0.01);
        assert!((b[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn master_loop_length_limits_new_tracks() {
        let sr = 48_000;
        let shared = SharedState::new(sr);
        let mut clock = Clock::new(sr);
        clock.set_loop(64);
        let mut track = Track::new(1, sr, 1.0);
        request(&mut track, &shared, &clock, TrackCommand::Record);
        for _ in 0..200 {
            track.process([0.1, 0.1], &params(), &clock);
            clock.tick();
        }
        assert_eq!(track.loop_len, 64);
        assert_eq!(track.state, TrackState::Playing);
        let _ = MAX_TRACKS;
    }
}
