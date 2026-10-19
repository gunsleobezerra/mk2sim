/**
 * Local simulation of the Rust backend.
 *
 * Used when the UI runs in a plain browser (`npm run dev`, static preview) so the
 * hardware replica stays fully interactive without the Tauri runtime. The state
 * shape and the command surface mirror `src-tauri/src/lib.rs` exactly.
 */

export const FX_KINDS = [
  "Off",
  "Delay",
  "Reverb",
  "LPF",
  "BPF",
  "HPF",
  "Phaser",
  "Flanger",
  "Pitch",
  "Beat Repeat",
  "Transpose",
  "Rev Delay",
  "Slicer",
  "Chorus",
  "Tremolo",
  "Lo-Fi",
];

export const QUANTIZE_LABELS = ["Off", "1/16", "1/8", "1/4", "1 Bar", "2 Bars"];
export const SYNC_LABELS = ["Immediate", "Measure", "Loop Length"];
export const DUB_MODE_LABELS = ["Overdub", "Replace1", "Replace2"];
export const GENRE_LABELS = ["Acoustic", "Rock", "Electro", "Guide"];
export const BEAT_LABELS = ["2/4", "3/4", "4/4", "5/4", "6/8", "7/8", "9/8", "12/8"];

const MAX_LOOP_MS = 20000;
const TICK_MS = 16;

const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

function createTrack(index) {
  return {
    id: index + 1,
    state: "Stopped",
    volume: 0.85,
    pan: 0,
    mute: false,
    reverse: false,
    oneShot: false,
    dubMode: "Overdub",
    fxA: 0,
    fxB: 0,
    fxLevel: 0.5,
    fill: 0,
    position: 0,
    peak: 0,
    recMs: 0,
    eqLow: 0,
    eqMid: 0,
    eqHigh: 0,
    comp: false,
    ns: false,
    loopMs: 0,
    phase: 0,
  };
}

export class Simulator {
  constructor() {
    this.tracks = Array.from({ length: 5 }, (_, index) => createTrack(index));
    this.bpm = 120;
    this.quantize = 3;
    this.syncMode = 1;
    this.memoryName = "MEMORY 01";
    this.inputMode = 0;
    this.inputFx = { a: 0, b: 0, level: 0.5 };
    this.rhythm = { enabled: false, genre: 0, pattern: 0, beat: 2, volume: 0.5 };
    this.tapTimes = [];
    this.clock = 0;
    this.listeners = new Set();
    this.timer = null;
    this.last = 0;
  }

  // -- lifecycle ------------------------------------------------------------

  subscribe(callback) {
    this.listeners.add(callback);
    this.start();
    callback(this.snapshot());
    return () => {
      this.listeners.delete(callback);
      if (this.listeners.size === 0) this.stop();
    };
  }

  start() {
    if (this.timer !== null) return;
    this.last = performance.now();
    this.timer = setInterval(() => this.tick(), TICK_MS);
  }

  stop() {
    if (this.timer === null) return;
    clearInterval(this.timer);
    this.timer = null;
  }

  emit() {
    const snapshot = this.snapshot();
    for (const listener of this.listeners) listener(snapshot);
  }

  // -- simulation -----------------------------------------------------------

  tick() {
    const now = performance.now();
    const dt = Math.min(now - this.last, 250);
    this.last = now;
    this.clock += dt;

    for (const track of this.tracks) {
      if (track.state === "Recording") {
        track.recMs = Math.min(track.recMs + dt, MAX_LOOP_MS);
        track.fill = track.recMs / MAX_LOOP_MS;
        track.peak = 0.35 + 0.4 * Math.abs(Math.sin(this.clock / 90 + track.id));
        if (track.recMs >= MAX_LOOP_MS) {
          track.loopMs = MAX_LOOP_MS;
          track.state = "Playing";
          track.phase = 0;
        }
      } else if (track.state === "Playing" || track.state === "Overdubbing") {
        const loop = track.loopMs || MAX_LOOP_MS;
        track.phase = (track.phase + dt / loop) % 1;
        track.position = track.phase;
        const envelope = 0.22 + 0.55 * Math.abs(Math.sin(track.phase * Math.PI * 3));
        track.peak = track.mute ? 0 : envelope * track.volume;
      } else {
        track.peak *= 0.9;
        if (track.peak < 0.001) track.peak = 0;
      }
    }

    this.emit();
  }

  // -- snapshot -------------------------------------------------------------

  snapshot() {
    const leader = this.tracks.find((track) => track.loopMs > 0);
    const position = leader ? leader.phase : 0;
    const loopLenMs = leader ? leader.loopMs : 0;

    return {
      bpm: Number(this.bpm.toFixed(1)),
      quantize: QUANTIZE_LABELS[this.quantize],
      syncMode: SYNC_LABELS[this.syncMode],
      memoryName: this.memoryName,
      position,
      loopLenMs,
      inputPeak: 0.08 + 0.35 * Math.abs(Math.sin(this.clock / 620)),
      cpu: 4 + 3 * Math.abs(Math.sin(this.clock / 1500)),
      backend: "simulator",
      device: "browser (no audio device)",
      engineOnline: false,
      inputMode: this.inputMode === 1 ? "Demo" : "Live",
      midiActivity: 0,
      midiPort: "",
      rhythm: {
        enabled: this.rhythm.enabled,
        genre: GENRE_LABELS[this.rhythm.genre],
        pattern: ["A", "B", "C", "D"][this.rhythm.pattern],
        beat: BEAT_LABELS[this.rhythm.beat],
        volume: this.rhythm.volume,
      },
      tracks: this.tracks.map((track) => ({
        id: track.id,
        state: track.state,
        volume: track.volume,
        pan: track.pan,
        mute: track.mute,
        reverse: track.reverse,
        oneShot: track.oneShot,
        dubMode: track.dubMode,
        fxA: track.fxA,
        fxB: track.fxB,
        fxLevel: track.fxLevel,
        fill: track.fill,
        position: track.position,
        peak: track.peak,
        recMs: Math.round(track.recMs),
        eqLow: track.eqLow,
        eqMid: track.eqMid,
        eqHigh: track.eqHigh,
        comp: track.comp,
        ns: track.ns,
      })),
      fxKinds: FX_KINDS,
    };
  }

  // -- commands -------------------------------------------------------------

  track(index) {
    if (!Number.isInteger(index) || index < 0 || index >= this.tracks.length) {
      throw new Error(`track ${index} out of range (0..5)`);
    }
    return this.tracks[index];
  }

  applyAction(track, action) {
    switch (action) {
      case "record":
      case "rec":
        if (track.state === "Recording") {
          track.state = "Playing";
          track.loopMs = Math.max(track.recMs, 250);
        } else {
          track.state = "Recording";
          track.recMs = 0;
          track.fill = 0;
          track.loopMs = 0;
          track.phase = 0;
        }
        break;
      case "play":
        if (track.state === "Recording") {
          track.state = "Playing";
          track.loopMs = Math.max(track.recMs, 250);
        } else if (track.loopMs > 0) {
          track.state = "Playing";
        }
        break;
      case "dub":
        if (track.loopMs > 0) {
          track.state = "Overdubbing";
          track.phase = track.phase || 0;
        }
        break;
      case "stop":
        track.state = "Stopped";
        track.phase = 0;
        track.position = 0;
        break;
      case "clear":
        track.state = "Stopped";
        track.loopMs = 0;
        track.recMs = 0;
        track.fill = 0;
        track.phase = 0;
        track.position = 0;
        break;
      case "undo":
        if (track.state === "Overdubbing") track.state = "Playing";
        break;
      case "redo":
        break;
      case "reverse":
        track.reverse = !track.reverse;
        break;
      case "oneshot":
      case "one_shot":
        track.oneShot = !track.oneShot;
        break;
      default:
        throw new Error(`unknown action '${action}'`);
    }
  }

  tapTempo() {
    const now = performance.now();
    if (this.tapTimes.length > 0 && now - this.tapTimes[this.tapTimes.length - 1] > 2500) {
      this.tapTimes.length = 0;
    }
    this.tapTimes.push(now);
    if (this.tapTimes.length > 6) this.tapTimes.shift();
    if (this.tapTimes.length < 2) return null;
    const gaps = [];
    for (let i = 1; i < this.tapTimes.length; i += 1) {
      gaps.push(this.tapTimes[i] - this.tapTimes[i - 1]);
    }
    const average = gaps.reduce((sum, gap) => sum + gap, 0) / gaps.length;
    this.bpm = clamp(60000 / average, 20, 300);
    return this.bpm;
  }

  async invoke(command, args = {}) {
    switch (command) {
      case "get_state":
        return this.snapshot();
      case "track_action":
        this.applyAction(this.track(args.track), args.action);
        break;
      case "transport_all":
        for (const track of this.tracks) this.applyAction(track, args.action);
        break;
      case "set_track_volume":
        this.track(args.track).volume = clamp(args.value, 0, 1);
        break;
      case "set_track_pan":
        this.track(args.track).pan = clamp(args.value, -1, 1);
        break;
      case "set_track_flag": {
        const track = this.track(args.track);
        const key = args.flag === "one_shot" ? "oneShot" : args.flag;
        // Whitelist mirrors `set_track_flag` in src-tauri/src/lib.rs.
        if (!["mute", "reverse", "oneShot", "comp", "ns"].includes(key)) {
          throw new Error(`unknown flag '${args.flag}'`);
        }
        track[key] = Boolean(args.value);
        break;
      }
      case "set_track_dub_mode":
        this.track(args.track).dubMode = DUB_MODE_LABELS[clamp(args.mode | 0, 0, 2)];
        break;
      case "set_track_fx": {
        const track = this.track(args.track);
        const slot = args.slot.toLowerCase() === "b" ? "fxB" : "fxA";
        track[slot] = clamp(args.kind | 0, 0, FX_KINDS.length - 1);
        break;
      }
      case "set_track_fx_level":
        this.track(args.track).fxLevel = clamp(args.value, 0, 1);
        break;
      case "set_track_eq": {
        const track = this.track(args.track);
        track.eqLow = clamp(args.low, -15, 15);
        track.eqMid = clamp(args.mid, -15, 15);
        track.eqHigh = clamp(args.high, -15, 15);
        break;
      }
      case "set_bpm":
        this.bpm = clamp(args.value, 20, 300);
        break;
      case "tap_tempo":
        return this.tapTempo();
      case "set_quantize":
        this.quantize = clamp(args.value | 0, 0, QUANTIZE_LABELS.length - 1);
        break;
      case "set_sync_mode":
        this.syncMode = clamp(args.value | 0, 0, SYNC_LABELS.length - 1);
        break;
      case "set_input_fx": {
        const slot = args.slot.toLowerCase() === "b" ? "b" : "a";
        this.inputFx[slot] = clamp(args.kind | 0, 0, FX_KINDS.length - 1);
        break;
      }
      case "set_input_fx_level":
        this.inputFx.level = clamp(args.value, 0, 1);
        break;
      case "set_memory_name":
        this.memoryName = String(args.name ?? "").slice(0, 16);
        break;
      case "set_rhythm":
        this.rhythm = {
          enabled: Boolean(args.enabled),
          genre: clamp(args.genre | 0, 0, GENRE_LABELS.length - 1),
          pattern: clamp(args.pattern | 0, 0, 3),
          beat: clamp(args.beat | 0, 0, BEAT_LABELS.length - 1),
          volume: clamp(args.volume, 0, 1),
        };
        break;
      case "set_input_mode":
        this.inputMode = clamp(args.mode | 0, 0, 1);
        break;
      case "list_midi_ports":
        return [];
      case "open_midi_port":
        return "";
      case "get_midi_mapping":
        return [];
      case "set_midi_mapping":
        return null;
      case "scan_plugins":
      case "list_plugins":
        return [];
      case "get_plugin_dirs":
        return ["%COMMONPROGRAMFILES%\\VST3", "%USERPROFILE%\\Documents\\VST3"];
      case "load_plugin":
        throw new Error("VST3 hosting requires the Tauri runtime");
      default:
        throw new Error(`unknown command '${command}'`);
    }
    this.emit();
    return null;
  }
}

export const simulator = new Simulator();
