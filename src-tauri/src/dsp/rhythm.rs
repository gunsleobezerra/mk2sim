//! Internal rhythm generator (drum machine) of the RC-505 MkII clone.
//!
//! Patterns are 16-step bitmasks, synthesized on the fly (kick, snare, hi-hat)
//! so no sample assets are required.

/// Pattern length in sixteenth notes for each supported time signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    TwoFour = 0,
    ThreeFour = 1,
    FourFour = 2,
    FiveFour = 3,
    SixEight = 4,
    SevenEight = 5,
    NineEight = 6,
    TwelveEight = 7,
}

impl Beat {
    pub const ALL: [Beat; 8] = [
        Beat::TwoFour,
        Beat::ThreeFour,
        Beat::FourFour,
        Beat::FiveFour,
        Beat::SixEight,
        Beat::SevenEight,
        Beat::NineEight,
        Beat::TwelveEight,
    ];

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Beat::TwoFour,
            1 => Beat::ThreeFour,
            3 => Beat::FiveFour,
            4 => Beat::SixEight,
            5 => Beat::SevenEight,
            6 => Beat::NineEight,
            7 => Beat::TwelveEight,
            _ => Beat::FourFour,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Beat::TwoFour => "2/4",
            Beat::ThreeFour => "3/4",
            Beat::FourFour => "4/4",
            Beat::FiveFour => "5/4",
            Beat::SixEight => "6/8",
            Beat::SevenEight => "7/8",
            Beat::NineEight => "9/8",
            Beat::TwelveEight => "12/8",
        }
    }

    pub fn steps_per_bar(self) -> usize {
        match self {
            Beat::TwoFour => 8,
            Beat::ThreeFour => 12,
            Beat::FourFour => 16,
            Beat::FiveFour => 20,
            Beat::SixEight => 12,
            Beat::SevenEight => 14,
            Beat::NineEight => 18,
            Beat::TwelveEight => 24,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Acoustic = 0,
    Rock = 1,
    Electro = 2,
    Guide = 3,
}

impl Genre {
    pub const ALL: [Genre; 4] = [Genre::Acoustic, Genre::Rock, Genre::Electro, Genre::Guide];

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Genre::Acoustic,
            1 => Genre::Rock,
            2 => Genre::Electro,
            _ => Genre::Guide,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Genre::Acoustic => "Acoustic",
            Genre::Rock => "Rock",
            Genre::Electro => "Electro",
            Genre::Guide => "Guide",
        }
    }

    fn masks(self) -> (u32, u32, u32) {
        // (kick, snare, hat)
        match self {
            Genre::Acoustic => (0b0001_0000_0001_0000, 0b0000_0100_0000_0100, 0b1010_1010_1010_1010),
            Genre::Rock => (0b1000_0001_1000_0001, 0b0000_0100_0000_0100, 0b0101_0101_0101_0101),
            Genre::Electro => (0b1000_0000_1000_0010, 0b0000_0000_0000_0100, 0b1111_1111_1111_1111),
            Genre::Guide => (0b1000_1000_1000_1000, 0, 0),
        }
    }
}

struct Voice {
    remaining: usize,
    phase: f32,
    step: f32,
    amp: f32,
    kind: VoiceKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VoiceKind {
    Kick,
    Snare,
    Hat,
}

struct XorShift(u32);

impl XorShift {
    fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }
    #[inline]
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        // Map to -1..1
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

pub struct RhythmEngine {
    pub enabled: bool,
    pub genre: Genre,
    pub pattern: u8, // A..D = 0..3
    pub beat: Beat,
    pub volume: f32,
    step_index: usize,
    samples_in_step: usize,
    voices: Vec<Voice>,
    rng: XorShift,
}

impl RhythmEngine {
    pub fn new() -> Self {
        Self {
            enabled: false,
            genre: Genre::Rock,
            pattern: 0,
            beat: Beat::FourFour,
            volume: 0.65,
            step_index: 0,
            samples_in_step: 0,
            voices: Vec::with_capacity(3),
            rng: XorShift::new(0x5050_5050),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.voices.clear();
            self.step_index = 0;
            self.samples_in_step = 0;
        }
    }

    pub fn reset(&mut self) {
        self.step_index = 0;
        self.samples_in_step = 0;
        self.voices.clear();
    }

    fn trigger(&mut self, kind: VoiceKind, sr: f32) {
        let (amp, step) = match kind {
            VoiceKind::Kick => (0.9, 45.0),
            VoiceKind::Snare => (0.55, 180.0),
            VoiceKind::Hat => (0.22, 8_000.0),
        };
        self.voices.push(Voice {
            remaining: (0.25 * sr) as usize,
            phase: 0.0,
            step,
            amp,
            kind,
        });
    }

    /// Renders one stereo frame.
    pub fn process(&mut self, sr: f32, bpm: f32) -> [f32; 2] {
        if !self.enabled {
            return [0.0; 2];
        }
        let step_len = ((60.0 / bpm.max(20.0)) * sr / 4.0).max(1.0) as usize;
        let (kick, snare, hat) = self.genre.masks();
        let bar_steps = self.beat.steps_per_bar().max(1);

        if self.samples_in_step == 0 {
            let step = self.step_index % bar_steps;
            let mask_step = step % 16;
            let variation = self.pattern & 0b11;
            let kick_on = (kick >> mask_step) & 1 == 1
                || (variation == 2 && mask_step % 8 == 6);
            let snare_on = (snare >> mask_step) & 1 == 1;
            let hat_on = (hat >> mask_step) & 1 == 1
                || (variation == 1 && mask_step % 2 == 1)
                || (variation == 3 && mask_step % 4 == 2);
            if kick_on {
                self.trigger(VoiceKind::Kick, sr);
            }
            if snare_on {
                self.trigger(VoiceKind::Snare, sr);
            }
            if hat_on {
                self.trigger(VoiceKind::Hat, sr);
            }
            self.step_index = (self.step_index + 1) % bar_steps;
        }

        self.samples_in_step += 1;
        if self.samples_in_step >= step_len {
            self.samples_in_step = 0;
        }

        let mut out = [0.0f32; 2];
        let mut any_finished = false;
        for voice in self.voices.iter_mut() {
            if voice.remaining == 0 {
                any_finished = true;
                continue;
            }
            voice.remaining -= 1;
            voice.phase += voice.step / sr;
            if voice.phase > 1.0 {
                voice.phase -= 1.0;
            }
            let env = (-(1.0 - voice.remaining as f32 / (0.25 * sr)) * 6.0).exp();
            let sample = match voice.kind {
                VoiceKind::Kick => (voice.phase * std::f32::consts::TAU).sin() * env * voice.amp,
                VoiceKind::Snare => {
                    let tone = (voice.phase * std::f32::consts::TAU).sin() * 0.35;
                    (tone + self.rng.next() * 0.65) * env * voice.amp
                }
                VoiceKind::Hat => self.rng.next() * env * voice.amp,
            };
            out[0] += sample;
            out[1] += sample;
        }
        if any_finished {
            self.voices.retain(|v| v.remaining > 0);
        }
        let gain = self.volume.clamp(0.0, 1.0);
        [out[0] * gain, out[1] * gain]
    }
}

impl Default for RhythmEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_labels_and_steps() {
        for beat in Beat::ALL {
            assert!(!beat.label().is_empty());
            assert!(beat.steps_per_bar() >= 8);
        }
        assert_eq!(Beat::FourFour.steps_per_bar(), 16);
    }

    #[test]
    fn enabled_rhythm_produces_sound() {
        let mut rhythm = RhythmEngine::new();
        rhythm.enabled = true;
        rhythm.genre = Genre::Rock;
        let sr = 48_000.0;
        let mut energy = 0.0f32;
        for _ in 0..(sr as usize) {
            let frame = rhythm.process(sr, 120.0);
            energy += frame[0].abs();
            assert!(frame[0].is_finite());
        }
        assert!(energy > 0.01, "rhythm should generate audible energy");
    }

    #[test]
    fn disabled_rhythm_is_silent() {
        let mut rhythm = RhythmEngine::new();
        rhythm.enabled = false;
        for _ in 0..1_000 {
            assert_eq!(rhythm.process(48_000.0, 120.0), [0.0, 0.0]);
        }
    }
}
