//! Native DSP effect chain used by Input FX and Track FX.
//!
//! Every effect is a small, allocation-free-in-`process` state machine. Buffers
//! are allocated when the effect is selected (from the UI thread) and reused for
//! the lifetime of the slot, so the audio callback never allocates.

use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Effect catalog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    None = 0,
    Delay = 1,
    Reverb = 2,
    Lpf = 3,
    Bpf = 4,
    Hpf = 5,
    Phaser = 6,
    Flanger = 7,
    PitchShift = 8,
    BeatRepeat = 9,
    Transpose = 10,
    ReverseDelay = 11,
    Slicer = 12,
    Chorus = 13,
    Tremolo = 14,
    Lofi = 15,
}

impl EffectKind {
    pub const ALL: [EffectKind; 16] = [
        EffectKind::None,
        EffectKind::Delay,
        EffectKind::Reverb,
        EffectKind::Lpf,
        EffectKind::Bpf,
        EffectKind::Hpf,
        EffectKind::Phaser,
        EffectKind::Flanger,
        EffectKind::PitchShift,
        EffectKind::BeatRepeat,
        EffectKind::Transpose,
        EffectKind::ReverseDelay,
        EffectKind::Slicer,
        EffectKind::Chorus,
        EffectKind::Tremolo,
        EffectKind::Lofi,
    ];

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => EffectKind::Delay,
            2 => EffectKind::Reverb,
            3 => EffectKind::Lpf,
            4 => EffectKind::Bpf,
            5 => EffectKind::Hpf,
            6 => EffectKind::Phaser,
            7 => EffectKind::Flanger,
            8 => EffectKind::PitchShift,
            9 => EffectKind::BeatRepeat,
            10 => EffectKind::Transpose,
            11 => EffectKind::ReverseDelay,
            12 => EffectKind::Slicer,
            13 => EffectKind::Chorus,
            14 => EffectKind::Tremolo,
            15 => EffectKind::Lofi,
            _ => EffectKind::None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EffectKind::None => "Off",
            EffectKind::Delay => "Delay",
            EffectKind::Reverb => "Reverb",
            EffectKind::Lpf => "LPF",
            EffectKind::Bpf => "BPF",
            EffectKind::Hpf => "HPF",
            EffectKind::Phaser => "Phaser",
            EffectKind::Flanger => "Flanger",
            EffectKind::PitchShift => "Pitch",
            EffectKind::BeatRepeat => "Beat Repeat",
            EffectKind::Transpose => "Transpose",
            EffectKind::ReverseDelay => "Rev Delay",
            EffectKind::Slicer => "Slicer",
            EffectKind::Chorus => "Chorus",
            EffectKind::Tremolo => "Tremolo",
            EffectKind::Lofi => "Lo-Fi",
        }
    }
}

// ---------------------------------------------------------------------------
// Effect trait + slot
// ---------------------------------------------------------------------------

pub trait DspEffect: Send {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32);
    fn reset(&mut self);
}

pub struct EffectSlot {
    kind: EffectKind,
    inner: Box<dyn DspEffect>,
}

impl EffectSlot {
    pub fn new(kind: EffectKind) -> Self {
        Self {
            kind,
            inner: build(kind),
        }
    }

    /// Idempotent; safe to call once per audio block.
    pub fn set_kind(&mut self, kind: EffectKind) {
        if kind != self.kind {
            self.kind = kind;
            self.inner = build(kind);
        }
    }

    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        self.inner.process(l, r, sr)
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

impl Default for EffectSlot {
    fn default() -> Self {
        Self::new(EffectKind::None)
    }
}

fn build(kind: EffectKind) -> Box<dyn DspEffect> {
    match kind {
        EffectKind::None => Box::new(Passthrough),
        EffectKind::Delay => Box::new(Delay::new(1.2, 0.28, 0.34, 0.38)),
        EffectKind::Reverb => Box::new(Reverb::new(0.38)),
        EffectKind::Lpf => Box::new(Svf::new(FilterMode::LowPass, 1_200.0, 0.9)),
        EffectKind::Bpf => Box::new(Svf::new(FilterMode::BandPass, 1_600.0, 1.4)),
        EffectKind::Hpf => Box::new(Svf::new(FilterMode::HighPass, 900.0, 0.9)),
        EffectKind::Phaser => Box::new(Phaser::new()),
        EffectKind::Flanger => Box::new(Flanger::new()),
        EffectKind::PitchShift => Box::new(PitchShift::new(1.5)),
        EffectKind::BeatRepeat => Box::new(BeatRepeat::new(0.25)),
        EffectKind::Transpose => Box::new(PitchShift::new(2f32.powf(-5.0 / 12.0))),
        EffectKind::ReverseDelay => Box::new(ReverseDelay::new(1.0)),
        EffectKind::Slicer => Box::new(Slicer::new(4)),
        EffectKind::Chorus => Box::new(Chorus::new()),
        EffectKind::Tremolo => Box::new(Tremolo::new(4.5)),
        EffectKind::Lofi => Box::new(Lofi::new(24.0, 6)),
    }
}

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

struct Passthrough;

impl DspEffect for Passthrough {
    fn process(&mut self, l: f32, r: f32, _sr: f32) -> (f32, f32) {
        (l, r)
    }
    fn reset(&mut self) {}
}

#[derive(Clone)]
struct DelayLine {
    buf: Vec<[f32; 2]>,
    idx: usize,
}

impl DelayLine {
    fn new(seconds: f32) -> Self {
        let len = (48_000.0 * seconds).max(64.0) as usize;
        Self {
            buf: vec![[0.0; 2]; len],
            idx: 0,
        }
    }

    #[inline]
    fn write(&mut self, sample: [f32; 2]) {
        self.buf[self.idx] = sample;
        self.idx = (self.idx + 1) % self.buf.len();
    }

    #[inline]
    fn read(&self, delay: usize) -> [f32; 2] {
        let len = self.buf.len();
        let d = delay.min(len - 1);
        self.buf[(self.idx + len - d) % len]
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = [0.0; 2]);
        self.idx = 0;
    }
}

struct Delay {
    line: DelayLine,
    seconds: f32,
    feedback: f32,
    mix: f32,
}

impl Delay {
    fn new(seconds: f32, time: f32, feedback: f32, mix: f32) -> Self {
        Self {
            line: DelayLine::new(seconds),
            seconds: time,
            feedback,
            mix,
        }
    }
}

impl DspEffect for Delay {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        let delay = (self.seconds * sr) as usize;
        let wet = self.line.read(delay);
        self.line.write([l + wet[0] * self.feedback, r + wet[1] * self.feedback]);
        (
            l * (1.0 - self.mix) + wet[0] * self.mix,
            r * (1.0 - self.mix) + wet[1] * self.mix,
        )
    }
    fn reset(&mut self) {
        self.line.reset();
    }
}

struct ReverseDelay {
    line: DelayLine,
    read: usize,
    feedback: f32,
}

impl ReverseDelay {
    fn new(seconds: f32) -> Self {
        let mut line = DelayLine::new(seconds);
        let read = line.buf.len() - 1;
        line.idx = 0;
        Self {
            line,
            read,
            feedback: 0.2,
        }
    }
}

impl DspEffect for ReverseDelay {
    fn process(&mut self, l: f32, r: f32, _sr: f32) -> (f32, f32) {
        let wet = self.line.buf[self.read];
        // Continuous backwards playback over the delay buffer.
        self.read = if self.read == 0 {
            self.line.buf.len() - 1
        } else {
            self.read - 1
        };
        self.line.write([l + wet[0] * self.feedback, r + wet[1] * self.feedback]);
        (l * 0.55 + wet[0] * 0.55, r * 0.55 + wet[1] * 0.55)
    }
    fn reset(&mut self) {
        self.line.reset();
        self.read = self.line.buf.len() - 1;
    }
}

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
    damp: f32,
    store: f32,
}

impl Comb {
    fn new(len: usize, feedback: f32, damp: f32) -> Self {
        Self {
            buf: vec![0.0; len.max(8)],
            idx: 0,
            feedback,
            damp,
            store: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = out * (1.0 - self.damp) + self.store * self.damp;
        self.buf[self.idx] = input + self.store * self.feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = 0.0);
        self.store = 0.0;
        self.idx = 0;
    }
}

struct AllPass {
    buf: Vec<f32>,
    idx: usize,
    gain: f32,
}

impl AllPass {
    fn new(len: usize, gain: f32) -> Self {
        Self {
            buf: vec![0.0; len.max(8)],
            idx: 0,
            gain,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.idx];
        let output = -input + bufout;
        self.buf[self.idx] = input + bufout * self.gain;
        self.idx = (self.idx + 1) % self.buf.len();
        output
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = 0.0);
        self.idx = 0;
    }
}

struct Reverb {
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    aps_l: Vec<AllPass>,
    aps_r: Vec<AllPass>,
    mix: f32,
}

impl Reverb {
    fn new(mix: f32) -> Self {
        // Schroeder/Freeverb-ish topology, scaled for a 48 kHz buffer.
        let comb_lengths = [1_557, 1_617, 1_491, 1_422];
        let ap_lengths = [225, 556];
        Self {
            combs_l: comb_lengths
                .iter()
                .map(|&l| Comb::new(l, 0.84, 0.2))
                .collect(),
            combs_r: comb_lengths
                .iter()
                .map(|&l| Comb::new(l + 23, 0.84, 0.2))
                .collect(),
            aps_l: ap_lengths.iter().map(|&l| AllPass::new(l, 0.5)).collect(),
            aps_r: ap_lengths
                .iter()
                .map(|&l| AllPass::new(l + 11, 0.5))
                .collect(),
            mix,
        }
    }
}

impl DspEffect for Reverb {
    fn process(&mut self, l: f32, r: f32, _sr: f32) -> (f32, f32) {
        let input = (l + r) * 0.5;
        let mut wet_l = 0.0;
        let mut wet_r = 0.0;
        for comb in &mut self.combs_l {
            wet_l += comb.process(input);
        }
        for comb in &mut self.combs_r {
            wet_r += comb.process(input);
        }
        wet_l *= 0.25;
        wet_r *= 0.25;
        for ap in &mut self.aps_l {
            wet_l = ap.process(wet_l);
        }
        for ap in &mut self.aps_r {
            wet_r = ap.process(wet_r);
        }
        (
            l * (1.0 - self.mix) + wet_l * self.mix,
            r * (1.0 - self.mix) + wet_r * self.mix,
        )
    }

    fn reset(&mut self) {
        self.combs_l.iter_mut().for_each(Comb::reset);
        self.combs_r.iter_mut().for_each(Comb::reset);
        self.aps_l.iter_mut().for_each(AllPass::reset);
        self.aps_r.iter_mut().for_each(AllPass::reset);
    }
}

#[derive(Clone, Copy)]
enum FilterMode {
    LowPass,
    BandPass,
    HighPass,
}

/// Topology-preserving state-variable filter (Zavalishin).
struct Svf {
    mode: FilterMode,
    cutoff: f32,
    q: f32,
    ic1: [f32; 2],
    ic2: [f32; 2],
}

impl Svf {
    fn new(mode: FilterMode, cutoff: f32, q: f32) -> Self {
        Self {
            mode,
            cutoff,
            q,
            ic1: [0.0; 2],
            ic2: [0.0; 2],
        }
    }

    #[inline]
    fn step(&mut self, x: f32, ch: usize, g: f32, k: f32) -> f32 {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2[ch];
        let v1 = a1 * self.ic1[ch] + a2 * v3;
        let v2 = self.ic2[ch] + a2 * self.ic1[ch] + a3 * v3;
        self.ic1[ch] = 2.0 * v1 - self.ic1[ch];
        self.ic2[ch] = 2.0 * v2 - self.ic2[ch];
        match self.mode {
            FilterMode::LowPass => v2,
            FilterMode::BandPass => v1,
            FilterMode::HighPass => x - k * v1 - v2,
        }
    }
}

impl DspEffect for Svf {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        let g = (PI * self.cutoff.min(sr * 0.45) / sr).tan();
        let k = 1.0 / self.q.max(0.1);
        (self.step(l, 0, g, k), self.step(r, 1, g, k))
    }

    fn reset(&mut self) {
        self.ic1 = [0.0; 2];
        self.ic2 = [0.0; 2];
    }
}

struct Phaser {
    phase: f32,
    z_l: [f32; 4],
    z_r: [f32; 4],
    feedback: f32,
}

impl Phaser {
    fn new() -> Self {
        Self {
            phase: 0.0,
            z_l: [0.0; 4],
            z_r: [0.0; 4],
            feedback: 0.15,
        }
    }

    #[inline]
    fn allpass(x: f32, z: &mut f32, a: f32) -> f32 {
        let y = -a * x + *z;
        *z = x + a * y;
        y
    }
}

impl DspEffect for Phaser {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        self.phase += (2.0 * PI * 0.25) / sr;
        if self.phase > 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        let sweep = 0.5 + 0.5 * self.phase.sin();
        let freq = 300.0 + sweep * 1_500.0;
        let t = (PI * freq / sr).tan();
        let a = (t - 1.0) / (t + 1.0);

        let mut yl = l + self.z_l[3] * self.feedback;
        let mut yr = r + self.z_r[3] * self.feedback;
        for i in 0..4 {
            yl = Self::allpass(yl, &mut self.z_l[i], a);
            yr = Self::allpass(yr, &mut self.z_r[i], a);
        }
        (l * 0.5 + yl * 0.5, r * 0.5 + yr * 0.5)
    }

    fn reset(&mut self) {
        self.z_l = [0.0; 4];
        self.z_r = [0.0; 4];
        self.phase = 0.0;
    }
}

struct Flanger {
    line: DelayLine,
    phase: f32,
    feedback: f32,
}

impl Flanger {
    fn new() -> Self {
        Self {
            line: DelayLine::new(0.05),
            phase: 0.0,
            feedback: 0.3,
        }
    }
}

impl DspEffect for Flanger {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        self.phase += (2.0 * PI * 0.35) / sr;
        if self.phase > 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        let mod_depth = (0.0015 + 0.0015 * (0.5 + 0.5 * self.phase.sin())) * sr;
        let wet = self.line.read(mod_depth as usize);
        self.line.write([
            l + wet[0] * self.feedback,
            r + wet[1] * self.feedback,
        ]);
        (l * 0.6 + wet[0] * 0.6, r * 0.6 + wet[1] * 0.6)
    }
    fn reset(&mut self) {
        self.line.reset();
        self.phase = 0.0;
    }
}

struct Chorus {
    line: DelayLine,
    phase: f32,
}

impl Chorus {
    fn new() -> Self {
        Self {
            line: DelayLine::new(0.08),
            phase: 0.0,
        }
    }
}

impl DspEffect for Chorus {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        self.phase += (2.0 * PI * 0.8) / sr;
        if self.phase > 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        let delay = (0.012 + 0.006 * (0.5 + 0.5 * self.phase.sin())) * sr;
        let wet = self.line.read(delay as usize);
        self.line.write([l, r]);
        (l * 0.65 + wet[0] * 0.55, r * 0.65 + wet[1] * 0.55)
    }
    fn reset(&mut self) {
        self.line.reset();
        self.phase = 0.0;
    }
}

struct PitchShift {
    buf: Vec<[f32; 2]>,
    write: usize,
    phase: f32,
    ratio: f32,
}

impl PitchShift {
    fn new(ratio: f32) -> Self {
        Self {
            buf: vec![[0.0; 2]; 4_096],
            write: 0,
            phase: 0.0,
            ratio,
        }
    }

    #[inline]
    fn read(&self, delay: usize) -> [f32; 2] {
        let len = self.buf.len();
        self.buf[(self.write + len - delay.min(len - 1)) % len]
    }
}

impl DspEffect for PitchShift {
    fn process(&mut self, l: f32, r: f32, _sr: f32) -> (f32, f32) {
        self.buf[self.write] = [l, r];
        self.write = (self.write + 1) % self.buf.len();

        let span = self.buf.len() as f32 * 0.5;
        let d1 = (self.phase * span) as usize + 1;
        let d2 = (((self.phase + 0.5) % 1.0) * span) as usize + 1;
        let w1 = (self.phase * PI).sin();
        let w2 = ((self.phase + 0.5) * PI).sin();

        let a = self.read(d1);
        let b = self.read(d2);
        self.phase = (self.phase + self.ratio - 1.0).rem_euclid(1.0);

        (
            a[0] * w1 + b[0] * w2,
            a[1] * w1 + b[1] * w2,
        )
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = [0.0; 2]);
        self.write = 0;
        self.phase = 0.0;
    }
}

struct BeatRepeat {
    capture: Vec<[f32; 2]>,
    pos: usize,
    filling: usize,
    seconds: f32,
}

impl BeatRepeat {
    fn new(seconds: f32) -> Self {
        Self {
            capture: vec![[0.0; 2]; 4_800],
            pos: 0,
            filling: 0,
            seconds,
        }
    }
}

impl DspEffect for BeatRepeat {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        let len = ((self.seconds * sr) as usize).clamp(64, self.capture.len());
        if self.filling < len {
            self.capture[self.filling] = [l, r];
            self.filling += 1;
            (l, r)
        } else {
            let out = self.capture[self.pos % len];
            self.capture[self.pos % len] = [l, r];
            self.pos = (self.pos + 1) % len;
            (out[0], out[1])
        }
    }

    fn reset(&mut self) {
        self.capture.iter_mut().for_each(|s| *s = [0.0; 2]);
        self.pos = 0;
        self.filling = 0;
    }
}

struct Slicer {
    counter: usize,
    divisor: usize,
}

impl Slicer {
    fn new(divisor: usize) -> Self {
        Self {
            counter: 0,
            divisor: divisor.max(1),
        }
    }
}

impl DspEffect for Slicer {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        let step = (sr / (self.divisor as f32 * 8.0)).max(1.0) as usize;
        self.counter = (self.counter + 1) % (step * 2);
        if self.counter < step {
            (l, r)
        } else {
            (0.0, 0.0)
        }
    }
    fn reset(&mut self) {
        self.counter = 0;
    }
}

struct Tremolo {
    phase: f32,
    rate: f32,
}

impl Tremolo {
    fn new(rate: f32) -> Self {
        Self { phase: 0.0, rate }
    }
}

impl DspEffect for Tremolo {
    fn process(&mut self, l: f32, r: f32, sr: f32) -> (f32, f32) {
        self.phase += (2.0 * PI * self.rate) / sr;
        if self.phase > 2.0 * PI {
            self.phase -= 2.0 * PI;
        }
        let gain = 0.5 + 0.5 * self.phase.sin();
        (l * gain, r * gain)
    }
    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

struct Lofi {
    hold: usize,
    counter: usize,
    levels: f32,
}

impl Lofi {
    fn new(levels: f32, hold: usize) -> Self {
        Self {
            hold: hold.max(1),
            counter: 0,
            levels,
        }
    }
}

impl DspEffect for Lofi {
    fn process(&mut self, l: f32, r: f32, _sr: f32) -> (f32, f32) {
        self.counter += 1;
        if self.counter % self.hold != 0 {
            return (l, r);
        }
        let quantize = |x: f32| (x * self.levels).round() / self.levels;
        (quantize(l), quantize(r))
    }
    fn reset(&mut self) {
        self.counter = 0;
    }
}

// ---------------------------------------------------------------------------
// Track utility processors: 3-band EQ + dynamics
// ---------------------------------------------------------------------------

pub struct ThreeBandEq {
    low_l: f32,
    low_r: f32,
    high_l: f32,
    high_r: f32,
}

impl ThreeBandEq {
    pub fn new() -> Self {
        Self {
            low_l: 0.0,
            low_r: 0.0,
            high_l: 0.0,
            high_r: 0.0,
        }
    }

    /// `low_db` / `mid_db` / `high_db` are gain trims in decibels (-15..+15).
    pub fn process(
        &mut self,
        l: f32,
        r: f32,
        sr: f32,
        low_db: f32,
        mid_db: f32,
        high_db: f32,
    ) -> (f32, f32) {
        let a_low = 1.0 - (-2.0 * PI * 200.0 / sr).exp();
        let a_high = 1.0 - (-2.0 * PI * 4_000.0 / sr).exp();
        self.low_l += a_low * (l - self.low_l);
        self.low_r += a_low * (r - self.low_r);
        self.high_l += a_high * (l - self.high_l);
        self.high_r += a_high * (r - self.high_r);

        let low_g = 10f32.powf(low_db / 20.0);
        let mid_g = 10f32.powf(mid_db / 20.0);
        let high_g = 10f32.powf(high_db / 20.0);

        let mid_l = l - self.low_l - (l - self.high_l);
        let mid_r = r - self.low_r - (r - self.high_r);

        (
            self.low_l * low_g + mid_l * mid_g + (l - self.high_l) * high_g,
            self.low_r * low_g + mid_r * mid_g + (r - self.high_r) * high_g,
        )
    }
}

impl Default for ThreeBandEq {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Dynamics {
    env: f32,
}

impl Dynamics {
    pub fn new() -> Self {
        Self { env: 0.0 }
    }

    pub fn process(&mut self, l: f32, r: f32, sr: f32, comp: bool, ns: bool) -> (f32, f32) {
        let level = (l.abs().max(r.abs())).max(1e-9);
        let attack = 1.0 - (-1.0 / (0.005 * sr)).exp();
        let release = 1.0 - (-1.0 / (0.120 * sr)).exp();
        let coef = if level > self.env { attack } else { release };
        self.env += coef * (level - self.env);

        let mut gain = 1.0;
        if comp && self.env > 0.35 {
            let over = self.env / 0.35;
            gain = 1.0 / over.powf(0.65);
        }
        if ns {
            let gate = ((self.env - 0.0015) / 0.004).clamp(0.0, 1.0);
            gain *= gate;
        }
        (l * gain, r * gain)
    }
}

impl Default for Dynamics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
    }

    #[test]
    fn effect_kinds_round_trip() {
        for kind in EffectKind::ALL {
            assert_eq!(EffectKind::from_u8(kind as u8), kind);
            assert!(!kind.label().is_empty());
        }
    }

    #[test]
    fn delay_produces_a_tail() {
        let mut slot = EffectSlot::new(EffectKind::Delay);
        let sr = 48_000.0;
        let mut out = Vec::new();
        for i in 0..(sr as usize) {
            let input = if i == 0 { 1.0 } else { 0.0 };
            let (l, _) = slot.process(input, input, sr);
            out.push(l);
        }
        assert!(out[0].abs() > 0.5, "dry impulse should pass through");
        // 0.28 s later the delayed impulse must appear.
        let delayed = out[(0.28 * sr) as usize];
        assert!(delayed.abs() > 0.05, "expected delayed impulse, got {delayed}");
    }

    #[test]
    fn lowpass_attenuates_high_frequencies() {
        let mut slot = EffectSlot::new(EffectKind::Lpf);
        let sr = 48_000.0;
        let mut high = Vec::new();
        let mut low = Vec::new();
        for i in 0..48_000 {
            let phase = 2.0 * PI * 8_000.0 * i as f32 / sr;
            let (l, _) = slot.process(phase.sin(), 0.0, sr);
            high.push(l);
        }
        slot.reset();
        for i in 0..48_000 {
            let phase = 2.0 * PI * 200.0 * i as f32 / sr;
            let (l, _) = slot.process(phase.sin(), 0.0, sr);
            low.push(l);
        }
        assert!(
            rms(&high[4_800..]) < rms(&low[4_800..]) * 0.5,
            "LPF should attenuate 8 kHz much more than 200 Hz"
        );
    }

    #[test]
    fn every_effect_stays_finite() {
        let sr = 48_000.0;
        for kind in EffectKind::ALL {
            let mut slot = EffectSlot::new(kind);
            for i in 0..8_000 {
                let x = (2.0 * PI * 220.0 * i as f32 / sr).sin() * 0.5;
                let (l, r) = slot.process(x, x, sr);
                assert!(l.is_finite() && r.is_finite(), "{kind:?} produced non-finite output");
                assert!(l.abs() < 32.0 && r.abs() < 32.0, "{kind:?} ran away");
            }
        }
    }

    #[test]
    fn eq_and_dynamics_are_finite() {
        let sr = 48_000.0;
        let mut eq = ThreeBandEq::new();
        let mut dyna = Dynamics::new();
        for i in 0..4_000 {
            let x = (2.0 * PI * 440.0 * i as f32 / sr).sin();
            let (l, r) = eq.process(x, x, sr, 6.0, -3.0, 4.0);
            let (l, r) = dyna.process(l, r, sr, true, true);
            assert!(l.is_finite() && r.is_finite());
        }
    }
}
