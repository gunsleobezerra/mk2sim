/**
 * Application shell: builds the hardware replica, wires every control to the
 * backend command surface and renders the 60 fps `rc505://state` stream.
 */

import { invoke, listen, isTauri } from "./lib/bridge.js";
import { el, fillSelect } from "./lib/dom.js";
import { QUANTIZE_LABELS, SYNC_LABELS } from "./lib/simulator.js";
import { createTrackStrip } from "./components/TrackStrip.js";
import { createLCD } from "./components/LCDDisplay.js";
import { createInputFxPanel } from "./components/FxPanel.js";
import { createRhythmPanel } from "./components/RhythmPanel.js";
import { createFxGrid } from "./components/FxGrid.js";

const toastElement = document.getElementById("toast");
let toastTimer = null;

function toast(message) {
  toastElement.textContent = message;
  toastElement.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastElement.classList.remove("show"), 3200);
}

async function guard(promise) {
  try {
    return await promise;
  } catch (error) {
    toast(String(error?.message ?? error));
    return null;
  }
}

let selectedTrack = 0;
let snapshot = null;

const api = {
  action: (track, action) => guard(invoke("track_action", { track, action })),
  transport: (action) => guard(invoke("transport_all", { action })),
  setVolume: (track, value) => guard(invoke("set_track_volume", { track, value })),
  setPan: (track, value) => guard(invoke("set_track_pan", { track, value })),
  setFlag: (track, flag, value) => guard(invoke("set_track_flag", { track, flag, value })),
  setDubMode: (track, mode) => guard(invoke("set_track_dub_mode", { track, mode })),
  setFx: (track, slot, kind) => guard(invoke("set_track_fx", { track, slot, kind })),
  setTrackFxLevel: (track, value) => guard(invoke("set_track_fx_level", { track, value })),
  setEq: (track, low, mid, high) => guard(invoke("set_track_eq", { track, low, mid, high })),
  setBpm: (value) => guard(invoke("set_bpm", { value })),
  setQuantize: (value) => guard(invoke("set_quantize", { value })),
  setSyncMode: (value) => guard(invoke("set_sync_mode", { value })),
  setInputFx: (slot, kind) => guard(invoke("set_input_fx", { slot, kind })),
  setInputFxLevel: (value) => guard(invoke("set_input_fx_level", { value })),
  setMemoryName: (name) => guard(invoke("set_memory_name", { name })),
  setRhythm: (state) =>
    guard(
      invoke("set_rhythm", {
        enabled: state.enabled,
        genre: state.genre,
        pattern: state.pattern,
        beat: state.beat,
        volume: state.volume,
      }),
    ),
  setInputMode: (mode) => guard(invoke("set_input_mode", { mode })),
  select: (index) => {
    selectedTrack = index;
    if (snapshot) render(snapshot);
  },
};

// ---------------------------------------------------------------------------
// Build the panel
// ---------------------------------------------------------------------------

const strips = Array.from({ length: 5 }, (_, index) => createTrackStrip({ index, api }));
document.getElementById("track-grid").append(...strips.map((strip) => strip.root));

const lcd = createLCD({ api });
document.getElementById("lcd-mount").append(lcd.root);

const inputFx = createInputFxPanel({ api });
document.getElementById("input-fx-panel").replaceWith(inputFx.root);

const rhythm = createRhythmPanel({ api });
document.getElementById("rhythm-panel").replaceWith(rhythm.root);

const fxGrid = createFxGrid({ api });
document.getElementById("fx-grid").replaceWith(fxGrid.root);

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

const bpmInput = document.getElementById("bpm-input");
bpmInput.addEventListener("change", () => {
  const value = Number(bpmInput.value);
  if (Number.isFinite(value)) api.setBpm(Math.min(300, Math.max(20, value)));
  bpmInput.blur();
});

document.getElementById("tap-tempo").addEventListener("click", () => guard(invoke("tap_tempo")));

const quantizeSelect = document.getElementById("quantize-select");
fillSelect(quantizeSelect, QUANTIZE_LABELS, 3);
quantizeSelect.addEventListener("change", () => api.setQuantize(Number(quantizeSelect.value)));

const syncSelect = document.getElementById("sync-select");
fillSelect(syncSelect, SYNC_LABELS, 1);
syncSelect.addEventListener("change", () => api.setSyncMode(Number(syncSelect.value)));

document
  .getElementById("input-mode-select")
  .addEventListener("change", (event) => api.setInputMode(Number(event.target.value)));

document.querySelectorAll("[data-transport]").forEach((button) => {
  button.addEventListener("click", () => api.transport(button.dataset.transport));
});

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

const midiPortSelect = document.getElementById("midi-port");
const midiConnectButton = document.getElementById("midi-connect");

async function refreshMidiPorts() {
  const ports = (await guard(invoke("list_midi_ports"))) ?? [];
  if (ports.length === 0) {
    midiPortSelect.replaceChildren(el("option", { value: "", text: "no MIDI inputs" }));
    return;
  }
  midiPortSelect.replaceChildren(
    ...ports.map((port) => el("option", { value: String(port.index), text: port.name })),
  );
}

document.getElementById("midi-refresh").addEventListener("click", refreshMidiPorts);
midiConnectButton.addEventListener("click", async () => {
  if (midiPortSelect.value === "") return;
  const name = await guard(invoke("open_midi_port", { index: Number(midiPortSelect.value) }));
  if (name) toast(`MIDI connected: ${name}`);
});

const pluginSelect = document.getElementById("plugin-list");

async function refreshPlugins() {
  const plugins = (await guard(invoke("scan_plugins"))) ?? [];
  if (plugins.length === 0) {
    pluginSelect.replaceChildren(el("option", { value: "", text: "no VST3 found" }));
    return;
  }
  pluginSelect.replaceChildren(
    ...plugins.map((plugin) =>
      el("option", {
        value: plugin.path,
        text: `${plugin.name}${plugin.loaded ? " ✓" : ""}`,
      }),
    ),
  );
}

document.getElementById("plugin-scan").addEventListener("click", refreshPlugins);
document.getElementById("plugin-load").addEventListener("click", async () => {
  const path = pluginSelect.value;
  if (!path) return;
  const descriptor = await guard(invoke("load_plugin", { path }));
  if (descriptor) toast(`Loaded ${descriptor.name} (${descriptor.format})`);
});

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

window.addEventListener("keydown", (event) => {
  const tag = event.target?.tagName;
  if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;

  if (event.code === "Space") {
    event.preventDefault();
    const state = snapshot?.tracks?.[selectedTrack]?.state ?? "Stopped";
    api.action(selectedTrack, state === "Stopped" ? "record" : "stop");
  } else if (/^Digit[1-5]$/.test(event.code)) {
    api.select(Number(event.code.slice(5)) - 1);
  } else if (event.code === "KeyQ") {
    api.transport("stop");
  }
});

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const ledEngine = document.getElementById("led-engine");
const ledMidi = document.getElementById("led-midi");
const backendLabel = document.getElementById("backend-label");
const inputMeter = document.getElementById("input-meter");
const cpuMeter = document.getElementById("cpu-meter");
const midiStatus = document.getElementById("midi-status");
const engineDetail = document.getElementById("engine-detail");

let lastMidiActivity = -1;
let midiBlink = 0;
let frameRequested = false;
let pendingSnapshot = null;

function schedule(next) {
  pendingSnapshot = next;
  if (frameRequested) return;
  frameRequested = true;
  requestAnimationFrame(flush);
}

function flush() {
  frameRequested = false;
  const next = pendingSnapshot;
  pendingSnapshot = null;
  if (!next) return;
  snapshot = next;
  render(next);
}

function render(next) {
  lcd.update(next);
  for (let index = 0; index < strips.length; index += 1) {
    const track = next.tracks[index];
    if (track) strips[index].update(track, { selected: index === selectedTrack });
  }
  fxGrid.update(next, selectedTrack);
  rhythm.update(next);

  if (document.activeElement !== bpmInput) {
    bpmInput.value = Number(next.bpm).toFixed(1);
  }
  const quantizeIndex = QUANTIZE_LABELS.indexOf(next.quantize);
  if (quantizeIndex >= 0 && document.activeElement !== quantizeSelect) {
    quantizeSelect.value = String(quantizeIndex);
  }
  const syncIndex = SYNC_LABELS.indexOf(next.syncMode);
  if (syncIndex >= 0 && document.activeElement !== syncSelect) {
    syncSelect.value = String(syncIndex);
  }

  ledEngine.classList.toggle("on", Boolean(next.engineOnline));
  ledEngine.classList.toggle("off", !next.engineOnline);
  backendLabel.textContent = `${String(next.backend).toUpperCase()} · ${Math.round(next.cpu)}%`;

  inputMeter.style.width = `${Math.min(100, Math.round(next.inputPeak * 120))}%`;
  cpuMeter.style.width = `${Math.min(100, Math.round(next.cpu * 2))}%`;

  if (next.midiActivity !== lastMidiActivity) {
    lastMidiActivity = next.midiActivity;
    midiBlink = 6;
  }
  if (midiBlink > 0) {
    midiBlink -= 1;
    ledMidi.classList.add("on");
  } else {
    ledMidi.classList.remove("on");
  }

  midiStatus.textContent = next.midiPort ? `MIDI: ${next.midiPort}` : "MIDI: not connected";
  engineDetail.textContent = `${next.device} · ${next.inputMode} in · ${isTauri ? "native" : "browser"}`;
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

refreshMidiPorts();
refreshPlugins();

listen("rc505://state", schedule).then((unlisten) => {
  window.addEventListener("beforeunload", () => unlisten());
});

guard(invoke("get_state")).then((initial) => {
  if (initial) schedule(initial);
});

if (!isTauri) {
  toast("Browser mode: audio engine simulated (run `npm run tauri dev` for CPAL)");
}
