/**
 * LCD display component — memory patch name, tempo, loop position, transport
 * grid and live engine diagnostics, mirroring the RC-505 MkII backlit LCD.
 */

import { el, formatTime } from "../lib/dom.js";

export function createLCD({ api }) {
  const memory = el("input", {
    class: "lcd-mem-input",
    type: "text",
    maxlength: 24,
    spellcheck: false,
    value: "MEMORY 01",
    "aria-label": "Memory patch name",
    onchange: () => api.setMemoryName(memory.value.trim() || "MEMORY 01"),
    onkeydown: (event) => {
      if (event.key === "Enter") memory.blur();
    },
  });

  const syncLabel = el("span", { text: "SYNC MEASURE" });
  const bpm = el("span", { class: "lcd-bpm", text: "120.0" });
  const time = el("span", { class: "lcd-time", text: "0:00.0" });
  const barFill = el("div", { class: "lcd-bar-fill" });
  const quantize = el("span", { text: "QUANTIZE 1/4" });
  const loop = el("span", { text: "LOOP --" });
  const cpu = el("span", { text: "CPU 0%" });
  const device = el("span", { text: "--" });

  const root = el(
    "div",
    { class: "lcd", role: "group", "aria-label": "Loop station display" },
    el("div", { class: "lcd-top" }, memory, syncLabel),
    el("div", { class: "lcd-main" }, bpm, el("span", { class: "lcd-unit", text: "BPM" }), time),
    el("div", { class: "lcd-bar" }, barFill),
    el("div", { class: "lcd-bottom" }, quantize, loop, cpu),
    el("div", { class: "lcd-bottom" }, device),
  );

  let lastMemory = null;

  function update(snapshot) {
    if (document.activeElement !== memory && snapshot.memoryName !== lastMemory) {
      lastMemory = snapshot.memoryName;
      memory.value = snapshot.memoryName;
    }

    bpm.textContent = Number(snapshot.bpm).toFixed(1);
    time.textContent = formatTime(snapshot.loopLenMs);
    barFill.style.width = `${Math.min(100, Math.max(0, snapshot.position * 100)).toFixed(1)}%`;
    syncLabel.textContent = `SYNC ${String(snapshot.syncMode).toUpperCase()}`;
    quantize.textContent = `QUANTIZE ${snapshot.quantize}`;
    loop.textContent = snapshot.loopLenMs > 0 ? `LOOP ${formatTime(snapshot.loopLenMs)}` : "LOOP --";
    cpu.textContent = `CPU ${Math.round(snapshot.cpu)}%`;
    device.textContent = `${String(snapshot.backend).toUpperCase()} · ${snapshot.device}`;
  }

  return { root, update };
}
