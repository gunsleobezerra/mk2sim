//! BOSS RC-505 MkII virtual clone — Tauri v2 application entry point.
//!
//! Architecture:
//! - `audio`   : CPAL engine, looper tracks, clock, VST3 host
//! - `dsp`     : native effects and the internal rhythm generator
//! - `midi`    : MIDI input, CC/ASSIGN mapping and clock
//! - `state`   : lock-free shared parameters between UI and audio thread
//!
//! The UI never touches the audio thread directly: commands go through a bounded
//! channel, parameters through atomics, and state comes back as a 60 fps event
//! stream (`rc505://state`).

pub mod audio;
pub mod dsp;
pub mod midi;
pub mod state;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{Emitter, Manager};

use audio::clock::{Clock, TapTempo};
use audio::engine::{self, EngineHandle};
use audio::plugin_host::{PluginDescriptor, PluginHost};
use dsp::effects::EffectKind;
use dsp::rhythm::{Beat, Genre};
use midi::controller::{CcMapping, MidiHub, MidiPort};
use state::{
    bits_to_f32, DubMode, Shared, SharedState, TrackCommand, TrackState, MAX_LOOP_SECONDS,
    MAX_TRACKS,
};

// ---------------------------------------------------------------------------
// UI snapshots
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrackSnapshot {
    pub id: usize,
    pub state: String,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub reverse: bool,
    pub one_shot: bool,
    pub dub_mode: String,
    pub fx_a: u8,
    pub fx_b: u8,
    pub fx_level: f32,
    pub fill: f32,
    pub position: f32,
    pub peak: f32,
    pub rec_ms: u32,
    pub eq_low: f32,
    pub eq_mid: f32,
    pub eq_high: f32,
    pub comp: bool,
    pub ns: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RhythmSnapshot {
    pub enabled: bool,
    pub genre: String,
    pub pattern: String,
    pub beat: String,
    pub volume: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub bpm: f32,
    pub quantize: String,
    pub sync_mode: String,
    pub memory_name: String,
    pub position: f32,
    pub loop_len_ms: f32,
    pub input_peak: f32,
    pub cpu: f32,
    pub backend: String,
    pub device: String,
    pub engine_online: bool,
    pub input_mode: String,
    pub midi_activity: u64,
    pub midi_port: String,
    pub rhythm: RhythmSnapshot,
    pub tracks: Vec<TrackSnapshot>,
    pub fx_kinds: Vec<String>,
}

pub struct AppState {
    pub shared: Shared,
    pub engine: EngineHandle,
    pub midi: Arc<MidiHub>,
    pub plugins: Arc<PluginHost>,
    pub tap: Mutex<TapTempo>,
}

fn build_snapshot(shared: &SharedState, midi: &MidiHub) -> Snapshot {
    let sample_rate = shared.sample_rate_value();
    let loop_len = shared.loop_len.load(Ordering::Relaxed);
    let position = shared.position.load(Ordering::Relaxed);
    let max_frames = sample_rate as f32 * MAX_LOOP_SECONDS;

    let tracks = (0..MAX_TRACKS)
        .map(|index| {
            let track = &shared.tracks[index];
            let track_state = TrackState::from_u8(track.state.load(Ordering::Relaxed));
            let track_loop = track.loop_len.load(Ordering::Relaxed);
            let play_pos = track.play_pos.load(Ordering::Relaxed);
            TrackSnapshot {
                id: index + 1,
                state: track_state.label().to_string(),
                volume: bits_to_f32(track.volume.load(Ordering::Relaxed)),
                pan: bits_to_f32(track.pan.load(Ordering::Relaxed)),
                mute: track.mute.load(Ordering::Relaxed),
                reverse: track.reverse.load(Ordering::Relaxed),
                one_shot: track.one_shot.load(Ordering::Relaxed),
                dub_mode: DubMode::from_u8(track.dub_mode.load(Ordering::Relaxed))
                    .label()
                    .to_string(),
                fx_a: track.fx_a.load(Ordering::Relaxed),
                fx_b: track.fx_b.load(Ordering::Relaxed),
                fx_level: bits_to_f32(track.fx_level.load(Ordering::Relaxed)),
                fill: (track_loop as f32 / max_frames).clamp(0.0, 1.0),
                position: if track_loop > 0 {
                    play_pos as f32 / track_loop as f32
                } else {
                    0.0
                },
                peak: bits_to_f32(track.peak.load(Ordering::Relaxed)),
                rec_ms: track.rec_ms.load(Ordering::Relaxed),
                eq_low: bits_to_f32(track.eq_low.load(Ordering::Relaxed)),
                eq_mid: bits_to_f32(track.eq_mid.load(Ordering::Relaxed)),
                eq_high: bits_to_f32(track.eq_high.load(Ordering::Relaxed)),
                comp: track.comp.load(Ordering::Relaxed),
                ns: track.ns.load(Ordering::Relaxed),
            }
        })
        .collect();

    Snapshot {
        bpm: shared.bpm_value(),
        quantize: shared.quantize_value().label().to_string(),
        sync_mode: shared.sync_mode_value().label().to_string(),
        memory_name: shared.memory_name.read().clone(),
        position: if loop_len > 0 {
            position as f32 / loop_len as f32
        } else {
            0.0
        },
        loop_len_ms: Clock::frames_to_ms(loop_len, sample_rate),
        input_peak: bits_to_f32(shared.input_peak.load(Ordering::Relaxed)),
        cpu: bits_to_f32(shared.cpu.load(Ordering::Relaxed)),
        backend: shared.backend.read().clone(),
        device: shared.device.read().clone(),
        engine_online: shared.engine_online.load(Ordering::Relaxed),
        input_mode: if shared.input_mode.load(Ordering::Relaxed) == 1 {
            "Demo".to_string()
        } else {
            "Live".to_string()
        },
        midi_activity: shared.midi_activity.load(Ordering::Relaxed),
        midi_port: midi.opened_port().unwrap_or_default(),
        rhythm: RhythmSnapshot {
            enabled: shared.rhythm_on.load(Ordering::Relaxed),
            genre: Genre::from_u8(shared.rhythm_genre.load(Ordering::Relaxed))
                .label()
                .to_string(),
            pattern: ["A", "B", "C", "D"][(shared.rhythm_pattern.load(Ordering::Relaxed) & 3)
                as usize]
                .to_string(),
            beat: Beat::from_u8(shared.rhythm_beat.load(Ordering::Relaxed))
                .label()
                .to_string(),
            volume: bits_to_f32(shared.rhythm_volume.load(Ordering::Relaxed)),
        },
        tracks,
        fx_kinds: EffectKind::ALL
            .iter()
            .map(|kind| kind.label().to_string())
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn track_index(track: usize) -> Result<usize, String> {
    if track < MAX_TRACKS {
        Ok(track)
    } else {
        Err(format!("track {track} out of range (0..{MAX_TRACKS})"))
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_state(state: tauri::State<'_, AppState>) -> Snapshot {
    build_snapshot(&state.shared, &state.midi)
}

#[tauri::command]
fn track_action(track: usize, action: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let index = track_index(track)?;
    let command = TrackCommand::parse(&action).ok_or_else(|| format!("unknown action '{action}'"))?;
    state
        .engine
        .tx
        .try_send((index, command))
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn transport_all(action: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let command = TrackCommand::parse(&action).ok_or_else(|| format!("unknown action '{action}'"))?;
    for index in 0..MAX_TRACKS {
        let _ = state.engine.tx.try_send((index, command));
    }
    Ok(())
}

#[tauri::command]
fn set_track_volume(track: usize, value: f32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let index = track_index(track)?;
    state.shared.tracks[index]
        .volume
        .store(state::f32_to_bits(value.clamp(0.0, 1.0)), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_track_pan(track: usize, value: f32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let index = track_index(track)?;
    state.shared.tracks[index]
        .pan
        .store(state::f32_to_bits(value.clamp(-1.0, 1.0)), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_track_flag(
    track: usize,
    flag: String,
    value: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let index = track_index(track)?;
    let shared = &state.shared.tracks[index];
    match flag.as_str() {
        "mute" => shared.mute.store(value, Ordering::Relaxed),
        "reverse" => shared.reverse.store(value, Ordering::Relaxed),
        "oneShot" | "one_shot" => shared.one_shot.store(value, Ordering::Relaxed),
        "comp" => shared.comp.store(value, Ordering::Relaxed),
        "ns" => shared.ns.store(value, Ordering::Relaxed),
        other => return Err(format!("unknown flag '{other}'")),
    }
    Ok(())
}

#[tauri::command]
fn set_track_dub_mode(track: usize, mode: u8, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let index = track_index(track)?;
    state.shared.tracks[index]
        .dub_mode
        .store(mode.min(2), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_track_fx(
    track: usize,
    slot: String,
    kind: u8,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let index = track_index(track)?;
    let target = match slot.as_str() {
        "a" | "A" => &state.shared.tracks[index].fx_a,
        "b" | "B" => &state.shared.tracks[index].fx_b,
        other => return Err(format!("unknown FX slot '{other}'")),
    };
    target.store(kind.min(EffectKind::ALL.len() as u8 - 1), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_track_fx_level(
    track: usize,
    value: f32,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let index = track_index(track)?;
    state.shared.tracks[index]
        .fx_level
        .store(state::f32_to_bits(value.clamp(0.0, 1.0)), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_track_eq(
    track: usize,
    low: f32,
    mid: f32,
    high: f32,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let index = track_index(track)?;
    let shared = &state.shared.tracks[index];
    shared
        .eq_low
        .store(state::f32_to_bits(low.clamp(-15.0, 15.0)), Ordering::Relaxed);
    shared
        .eq_mid
        .store(state::f32_to_bits(mid.clamp(-15.0, 15.0)), Ordering::Relaxed);
    shared
        .eq_high
        .store(state::f32_to_bits(high.clamp(-15.0, 15.0)), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_bpm(value: f32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.shared.set_bpm(value);
    Ok(())
}

#[tauri::command]
fn tap_tempo(state: tauri::State<'_, AppState>) -> Option<f32> {
    let bpm = state.tap.lock().tap();
    if let Some(value) = bpm {
        state.shared.set_bpm(value);
    }
    bpm
}

#[tauri::command]
fn set_quantize(value: u8, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .shared
        .quantize
        .store(value.min(5), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_sync_mode(value: u8, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .shared
        .sync_mode
        .store(value.min(2), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_input_fx(slot: String, kind: u8, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let target = match slot.as_str() {
        "a" | "A" => &state.shared.input_fx_a,
        "b" | "B" => &state.shared.input_fx_b,
        other => return Err(format!("unknown input FX slot '{other}'")),
    };
    target.store(kind.min(EffectKind::ALL.len() as u8 - 1), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_input_fx_level(value: f32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .shared
        .input_fx_level
        .store(state::f32_to_bits(value.clamp(0.0, 1.0)), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_memory_name(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.shared.set_memory_name(&name);
    Ok(())
}

#[tauri::command]
fn set_rhythm(
    enabled: bool,
    genre: u8,
    pattern: u8,
    beat: u8,
    volume: f32,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.shared.rhythm_on.store(enabled, Ordering::Relaxed);
    state
        .shared
        .rhythm_genre
        .store(genre.min(3), Ordering::Relaxed);
    state
        .shared
        .rhythm_pattern
        .store(pattern & 3, Ordering::Relaxed);
    state
        .shared
        .rhythm_beat
        .store(beat.min(7), Ordering::Relaxed);
    state
        .shared
        .rhythm_volume
        .store(state::f32_to_bits(volume.clamp(0.0, 1.0)), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_input_mode(mode: u8, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.shared.input_mode.store(mode.min(1), Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn list_midi_ports(state: tauri::State<'_, AppState>) -> Vec<MidiPort> {
    state.midi.list_ports()
}

#[tauri::command]
fn open_midi_port(index: usize, state: tauri::State<'_, AppState>) -> Result<String, String> {
    state.midi.open_port(index)
}

#[tauri::command]
fn get_midi_mapping(state: tauri::State<'_, AppState>) -> Vec<CcMapping> {
    state.midi.mapping()
}

#[tauri::command]
fn set_midi_mapping(mapping: Vec<CcMapping>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.midi.set_mapping(mapping);
    Ok(())
}

#[tauri::command]
fn scan_plugins(state: tauri::State<'_, AppState>) -> Vec<PluginDescriptor> {
    state.plugins.scan()
}

#[tauri::command]
fn list_plugins(state: tauri::State<'_, AppState>) -> Vec<PluginDescriptor> {
    state.plugins.list()
}

#[tauri::command]
fn load_plugin(path: String, state: tauri::State<'_, AppState>) -> Result<PluginDescriptor, String> {
    state.plugins.load(&path)
}

#[tauri::command]
fn get_plugin_dirs(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.plugins.dirs()
}

// ---------------------------------------------------------------------------
// Application bootstrap
// ---------------------------------------------------------------------------

pub fn run() {
    let shared = Arc::new(SharedState::new(48_000));
    let engine = engine::spawn(shared.clone());
    let midi = Arc::new(MidiHub::new(engine.tx.clone(), shared.clone()));
    let plugins = Arc::new(PluginHost::new());

    let app_state = AppState {
        shared: shared.clone(),
        engine,
        midi: midi.clone(),
        plugins,
        tap: Mutex::new(TapTempo::default()),
    };

    let emit_shared = shared.clone();
    let emit_midi = midi.clone();

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let shared = emit_shared.clone();
            let midi = emit_midi.clone();
            thread::Builder::new()
                .name("rc505-ui-events".to_string())
                .spawn(move || loop {
                    thread::sleep(Duration::from_millis(16));
                    let snapshot = build_snapshot(&shared, &midi);
                    if let Err(err) = handle.emit("rc505://state", &snapshot) {
                        eprintln!("[ipc] failed to emit state: {err}");
                    }
                })
                .expect("failed to spawn UI event thread");

            // Keep the main window reachable for debugging.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("BOSS RC-505 MkII — Virtual Clone");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            track_action,
            transport_all,
            set_track_volume,
            set_track_pan,
            set_track_flag,
            set_track_dub_mode,
            set_track_fx,
            set_track_fx_level,
            set_track_eq,
            set_bpm,
            tap_tempo,
            set_quantize,
            set_sync_mode,
            set_input_fx,
            set_input_fx_level,
            set_memory_name,
            set_rhythm,
            set_input_mode,
            list_midi_ports,
            open_midi_port,
            get_midi_mapping,
            set_midi_mapping,
            scan_plugins,
            list_plugins,
            load_plugin,
            get_plugin_dirs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RC-505 clone");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Quantize, SyncMode};

    #[test]
    fn snapshot_has_five_tracks_and_fx_catalog() {
        let shared = SharedState::new(48_000);
        let (tx, _rx) = crossbeam_channel::bounded(8);
        let midi = MidiHub::new(tx, Arc::new(SharedState::new(48_000)));
        let snapshot = build_snapshot(&shared, &midi);
        assert_eq!(snapshot.tracks.len(), MAX_TRACKS);
        assert_eq!(snapshot.tracks[0].id, 1);
        assert_eq!(snapshot.fx_kinds.len(), EffectKind::ALL.len());
        assert_eq!(snapshot.quantize, Quantize::Quarter.label());
        assert_eq!(snapshot.sync_mode, SyncMode::Measure.label());
    }

    #[test]
    fn track_index_validation() {
        assert!(track_index(0).is_ok());
        assert!(track_index(MAX_TRACKS - 1).is_ok());
        assert!(track_index(MAX_TRACKS).is_err());
    }
}
