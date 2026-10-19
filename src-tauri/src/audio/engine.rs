//! Real-time audio engine.
//!
//! The engine prefers a real CPAL device (WASAPI/ASIO on Windows, CoreAudio on
//! macOS, ALSA/JACK on Linux). When no device is available it falls back to a
//! paced software renderer so the looper state machine and the UI keep working
//! (useful in CI and on machines without an audio interface).
//!
//! Communication with the UI is lock-free: parameters live in atomics and state
//! transitions travel through a bounded crossbeam channel.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use crossbeam_channel::{bounded, Receiver, Sender};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapRb};

use crate::audio::clock::Clock;
use crate::audio::track::Track;
use crate::dsp::effects::{EffectKind, EffectSlot};
use crate::dsp::rhythm::{Beat, Genre, RhythmEngine};
use crate::state::{
    bits_to_f32, f32_to_bits, Shared, SharedState, TrackBlockParams, TrackCommand, TrackState,
    MAX_LOOP_SECONDS, MAX_TRACKS,
};

pub struct EngineHandle {
    pub shared: Shared,
    pub tx: Sender<(usize, TrackCommand)>,
}

impl EngineHandle {
    pub fn send(&self, track: usize, command: TrackCommand) {
        let _ = self.tx.try_send((track, command));
    }
}

/// Starts the audio thread. Never blocks the caller.
pub fn spawn(shared: Shared) -> EngineHandle {
    let (tx, rx) = bounded::<(usize, TrackCommand)>(512);
    let shared_thread = shared.clone();
    thread::Builder::new()
        .name("rc505-audio".to_string())
        .spawn(move || {
            if let Err(err) = run_device(shared_thread.clone(), rx.clone()) {
                eprintln!("[audio] device backend unavailable: {err}; using software renderer");
                *shared_thread.backend.write() = "software".to_string();
                *shared_thread.device.write() = "virtual".to_string();
                run_software(shared_thread, rx);
            }
        })
        .expect("failed to spawn audio thread");

    EngineHandle { shared, tx }
}

// ---------------------------------------------------------------------------
// CPAL backend
// ---------------------------------------------------------------------------

fn run_device(shared: Shared, rx: Receiver<(usize, TrackCommand)>) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no default output device"))?;
    let supported = device.default_output_config()?;
    let sample_rate = supported.sample_rate().0;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.config();
    let channels = config.channels as usize;

    shared.sample_rate.store(sample_rate, Ordering::Relaxed);
    *shared.backend.write() = "cpal".to_string();
    *shared.device.write() = device.name().unwrap_or_else(|_| "default output".to_string());

    // Input capture (best effort). Mono input is duplicated to stereo.
    let ring = HeapRb::<f32>::new(sample_rate as usize * 2);
    let (producer, consumer) = ring.split();
    let input_stream = match build_input(host.default_input_device(), sample_rate, producer) {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("[audio] no live input ({err}); demo tone enabled");
            shared.input_mode.store(1, Ordering::Relaxed);
            None
        }
    };

    let stream = match sample_format {
        cpal::SampleFormat::F32 => build_output::<f32>(&device, &config, shared.clone(), rx, consumer, sample_rate),
        cpal::SampleFormat::I16 => build_output::<i16>(&device, &config, shared.clone(), rx, consumer, sample_rate),
        cpal::SampleFormat::U16 => build_output::<u16>(&device, &config, shared.clone(), rx, consumer, sample_rate),
        other => Err(anyhow!("unsupported output sample format {other:?}")),
    }?;

    stream.play()?;
    if let Some(input) = input_stream.as_ref() {
        input.play()?;
    }
    shared.engine_online.store(true, Ordering::Relaxed);
    eprintln!(
        "[audio] running on '{}' @ {} Hz / {} ch",
        *shared.device.read(),
        sample_rate,
        channels
    );

    // Keep the streams alive for the lifetime of the process.
    loop {
        thread::sleep(Duration::from_secs(3_600));
    }
}

fn build_input(
    device: Option<cpal::Device>,
    sample_rate: u32,
    producer: ringbuf::HeapProd<f32>,
) -> Result<Option<cpal::Stream>> {
    let device = match device {
        Some(device) => device,
        None => return Ok(None),
    };
    let supported = device.default_input_config()?;
    if supported.sample_rate().0 != sample_rate {
        return Err(anyhow!(
            "input sample rate {} != output {}",
            supported.sample_rate().0,
            sample_rate
        ));
    }
    let config: cpal::StreamConfig = supported.config();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_input_stream::<f32>(&device, &config, producer),
        cpal::SampleFormat::I16 => build_input_stream::<i16>(&device, &config, producer),
        cpal::SampleFormat::U16 => build_input_stream::<u16>(&device, &config, producer),
        other => Err(anyhow!("unsupported input sample format {other:?}")),
    }?;
    Ok(Some(stream))
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut producer: ringbuf::HeapProd<f32>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + Sample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("[audio] input error: {err}");
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            if channels == 0 {
                return;
            }
            for frame in data.chunks(channels) {
                let left = f32::from_sample(frame[0]);
                let right = if channels > 1 {
                    f32::from_sample(frame[1])
                } else {
                    left
                };
                let _ = producer.try_push(left);
                let _ = producer.try_push(right);
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn build_output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Shared,
    rx: Receiver<(usize, TrackCommand)>,
    mut input: HeapCons<f32>,
    sample_rate: u32,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + Sample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut renderer = Renderer::new(sample_rate);
    // Fixed scratch buffers: the audio callback never allocates.
    let mut input_buf = [[0.0f32; 2]; 4_096];
    let mut output_buf = [0.0f32; 8_192];

    let err_fn = |err| eprintln!("[audio] output error: {err}");
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            if channels == 0 {
                return;
            }
            let frames = (data.len() / channels).min(input_buf.len());
            for frame in input_buf.iter_mut().take(frames) {
                let left = input.try_pop().unwrap_or(0.0);
                let right = input.try_pop().unwrap_or(left);
                *frame = [left, right];
            }
            renderer.render(
                &mut output_buf[..frames * 2],
                &input_buf[..frames],
                &shared,
                &rx,
            );
            for i in 0..frames {
                let left = T::from_sample(output_buf[i * 2]);
                let right = T::from_sample(output_buf[i * 2 + 1]);
                for ch in 0..channels {
                    data[i * channels + ch] = if ch == 0 { left } else { right };
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

// ---------------------------------------------------------------------------
// Software fallback backend
// ---------------------------------------------------------------------------

fn run_software(shared: Shared, rx: Receiver<(usize, TrackCommand)>) {
    let sample_rate = shared.sample_rate_value();
    let mut renderer = Renderer::new(sample_rate);
    let block_frames = (sample_rate / 100) as usize; // 10 ms
    let mut input = vec![[0.0f32; 2]; block_frames];
    let mut output = vec![0.0f32; block_frames * 2];
    let block_time = Duration::from_secs_f64(block_frames as f64 / sample_rate as f64);

    shared.engine_online.store(true, Ordering::Relaxed);
    eprintln!("[audio] software renderer @ {sample_rate} Hz");

    loop {
        let start = Instant::now();
        for frame in input.iter_mut() {
            *frame = [0.0; 2];
        }
        renderer.render(&mut output, &input, &shared, &rx);
        let elapsed = start.elapsed();
        if elapsed < block_time {
            thread::sleep(block_time - elapsed);
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer (shared by both backends, fully testable without hardware)
// ---------------------------------------------------------------------------

pub struct Renderer {
    clock: Clock,
    tracks: Vec<Track>,
    input_fx_a: EffectSlot,
    input_fx_b: EffectSlot,
    rhythm: RhythmEngine,
    demo_phase: f32,
    demo_phase2: f32,
}

impl Renderer {
    pub fn new(sample_rate: u32) -> Self {
        let tracks = (0..MAX_TRACKS)
            .map(|id| Track::new(id, sample_rate, MAX_LOOP_SECONDS))
            .collect();
        Self {
            clock: Clock::new(sample_rate),
            tracks,
            input_fx_a: EffectSlot::new(EffectKind::None),
            input_fx_b: EffectSlot::new(EffectKind::None),
            rhythm: RhythmEngine::new(),
            demo_phase: 0.0,
            demo_phase2: 0.0,
        }
    }

    pub fn render(
        &mut self,
        out: &mut [f32],
        input: &[[f32; 2]],
        shared: &SharedState,
        rx: &Receiver<(usize, TrackCommand)>,
    ) {
        let sample_rate = shared.sample_rate_value();
        let bpm = shared.bpm_value();
        self.clock.configure(sample_rate, bpm);
        self.clock
            .set_loop(shared.loop_len.load(Ordering::Relaxed));

        let quantize = shared.quantize_value();
        let sync = shared.sync_mode_value();

        // 1. Drain pending state-machine commands.
        while let Ok((index, command)) = rx.try_recv() {
            if let Some(track) = self.tracks.get_mut(index) {
                track.request(command, &self.clock, quantize, sync, &shared.tracks[index]);
            }
        }

        // 2. Block-rate parameter snapshot.
        let params: [TrackBlockParams; MAX_TRACKS] =
            std::array::from_fn(|i| shared.tracks[i].snapshot());
        self.input_fx_a
            .set_kind(EffectKind::from_u8(shared.input_fx_a.load(Ordering::Relaxed)));
        self.input_fx_b
            .set_kind(EffectKind::from_u8(shared.input_fx_b.load(Ordering::Relaxed)));
        let input_fx_level = bits_to_f32(shared.input_fx_level.load(Ordering::Relaxed));
        let demo_mode = shared.input_mode.load(Ordering::Relaxed) == 1;

        self.rhythm.genre = Genre::from_u8(shared.rhythm_genre.load(Ordering::Relaxed));
        self.rhythm.pattern = shared.rhythm_pattern.load(Ordering::Relaxed);
        self.rhythm.beat = Beat::from_u8(shared.rhythm_beat.load(Ordering::Relaxed));
        self.rhythm.volume = bits_to_f32(shared.rhythm_volume.load(Ordering::Relaxed));
        self.rhythm
            .set_enabled(shared.rhythm_on.load(Ordering::Relaxed));

        let frames = out.len() / 2;
        let sr = sample_rate as f32;
        let mut input_peak = 0.0f32;
        let started = Instant::now();

        for i in 0..frames {
            let mut frame = input.get(i).copied().unwrap_or([0.0; 2]);
            if demo_mode {
                frame = self.demo_tone(sample_rate);
            }
            input_peak = input_peak.max(frame[0].abs()).max(frame[1].abs());

            let (mut l, mut r) = self.input_fx_a.process(frame[0], frame[1], sr);
            (l, r) = self.input_fx_b.process(l, r, sr);
            let frame = [
                frame[0] * (1.0 - input_fx_level) + l * input_fx_level,
                frame[1] * (1.0 - input_fx_level) + r * input_fx_level,
            ];

            self.clock.tick();

            // Direct monitoring: the RC-505 passes the processed input straight to
            // the outputs so the player always hears the instrument.
            let mut mix_l = frame[0];
            let mut mix_r = frame[1];
            for (index, track) in self.tracks.iter_mut().enumerate() {
                track.tick(&self.clock, &shared.tracks[index]);
                let out = track.process(frame, &params[index], &self.clock);
                mix_l += out[0];
                mix_r += out[1];
            }

            let rhythm = self.rhythm.process(sr, self.clock.bpm);
            mix_l += rhythm[0];
            mix_r += rhythm[1];

            out[i * 2] = soft_clip(mix_l);
            out[i * 2 + 1] = soft_clip(mix_r);
        }

        // 3. Publish state back to the UI.
        for (index, track) in self.tracks.iter_mut().enumerate() {
            let target = &shared.tracks[index];
            target
                .state
                .store(track.state as u8, Ordering::Relaxed);
            target
                .loop_len
                .store(track.loop_len as u64, Ordering::Relaxed);
            target
                .play_pos
                .store(track.play_pos as u64, Ordering::Relaxed);
            target
                .peak
                .store(f32_to_bits(track.peak), Ordering::Relaxed);
            let rec_ms = if track.state == TrackState::Recording {
                track.rec_pos as f32 / sr * 1_000.0
            } else {
                Clock::frames_to_ms(track.loop_len as u64, sample_rate)
            };
            target.rec_ms.store(rec_ms as u32, Ordering::Relaxed);

            if let Some(len) = track.take_finalized() {
                if shared.loop_len.load(Ordering::Relaxed) == 0 {
                    shared.loop_len.store(len as u64, Ordering::Relaxed);
                }
            }
            if track.take_cleared() {
                shared.loop_len.store(0, Ordering::Relaxed);
            }
        }

        shared
            .position
            .store(self.clock.position, Ordering::Relaxed);
        shared
            .input_peak
            .store(f32_to_bits(input_peak.min(1.0)), Ordering::Relaxed);

        let block_seconds = frames as f32 / sr;
        let cpu = if block_seconds > 0.0 {
            started.elapsed().as_secs_f32() / block_seconds
        } else {
            0.0
        };
        shared
            .cpu
            .store(f32_to_bits(cpu.clamp(0.0, 4.0)), Ordering::Relaxed);
    }

    fn demo_tone(&mut self, sample_rate: u32) -> [f32; 2] {
        // Deterministic two-tone test signal with a slow amplitude envelope, used
        // when no input device exists so the looper remains demonstrable.
        let sr = sample_rate as f32;
        self.demo_phase = (self.demo_phase + 220.0 / sr).fract();
        self.demo_phase2 = (self.demo_phase2 + 331.0 / sr).fract();
        let env = 0.55 + 0.45 * (self.demo_phase * std::f32::consts::TAU * 0.25).sin();
        let value = (self.demo_phase * std::f32::consts::TAU).sin() * 0.22 * env
            + (self.demo_phase2 * std::f32::consts::TAU).sin() * 0.08 * env;
        [value, value]
    }
}

#[inline]
fn soft_clip(x: f32) -> f32 {
    let clamped = x.clamp(-3.0, 3.0);
    clamped / (1.0 + clamped.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Quantize, SharedState, SyncMode};

    /// Drives the shared atomics into "no timing correction" mode so tests are
    /// deterministic instead of waiting for a measure boundary.
    fn immediate(shared: &SharedState) {
        shared.quantize.store(Quantize::Off as u8, Ordering::Relaxed);
        shared
            .sync_mode
            .store(SyncMode::Immediate as u8, Ordering::Relaxed);
    }

    fn render_block(renderer: &mut Renderer, shared: &SharedState, rx: &Receiver<(usize, TrackCommand)>) -> Vec<f32> {
        let mut out = vec![0.0f32; 480 * 2];
        let input = vec![[0.25f32; 2]; 480];
        renderer.render(&mut out, &input, shared, rx);
        out
    }

    #[test]
    fn renderer_records_and_plays_back() {
        let shared = SharedState::new(48_000);
        immediate(&shared);
        let (tx, rx) = bounded(64);
        let mut renderer = Renderer::new(48_000);

        tx.send((0, TrackCommand::Record)).unwrap();
        for _ in 0..10 {
            render_block(&mut renderer, &shared, &rx);
        }
        tx.send((0, TrackCommand::Play)).unwrap();
        render_block(&mut renderer, &shared, &rx);

        assert_eq!(
            shared.tracks[0].state.load(Ordering::Relaxed),
            TrackState::Playing as u8
        );
        assert!(shared.loop_len.load(Ordering::Relaxed) > 0);

        let mut heard = 0.0f32;
        for _ in 0..4 {
            let block = render_block(&mut renderer, &shared, &rx);
            heard += block.iter().map(|s| s.abs()).sum::<f32>();
        }
        assert!(heard > 0.1, "playback should produce audible output");
    }

    #[test]
    fn demo_mode_overrides_silent_input() {
        let shared = SharedState::new(48_000);
        shared.input_mode.store(1, Ordering::Relaxed);
        let (_tx, rx) = bounded(8);
        let mut renderer = Renderer::new(48_000);
        let mut out = vec![0.0f32; 480 * 2];
        let input = vec![[0.0f32; 2]; 480];
        renderer.render(&mut out, &input, &shared, &rx);
        assert!(
            out.iter().any(|s| s.abs() > 1e-4),
            "demo tone should be audible"
        );
    }

    #[test]
    fn soft_clip_stays_bounded() {
        for x in [-10.0, -1.0, 0.0, 1.0, 10.0] {
            assert!(soft_clip(x).abs() <= 1.0);
        }
    }
}
