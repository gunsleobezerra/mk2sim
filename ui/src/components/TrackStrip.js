/**
 * Track strip component — the physical column of the RC-505 MkII.
 *
 * Owns: LED ring (state colour + loop position arc), big rec/play/dub button,
 * stop/clear/undo/redo cluster, volume fader, pan knob, level meter, FX A/B
 * selectors and the MUTE / REVERSE / 1SHOT toggles.
 */

import { attachKnob, el, fillSelect } from "../lib/dom.js";
import { FX_KINDS } from "../lib/simulator.js";

export const TRACK_COLORS = ["#ff4b3e", "#ffb02e", "#41d67a", "#39c6ff", "#b06bff"];

const RING_RADIUS = 44;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

const BIG_BUTTON_LABEL = {
  Stopped: "REC",
  Recording: "PLAY",
  Playing: "DUB",
  Overdubbing: "STOP",
};

const NEXT_ACTION = {
  Stopped: "record",
  Recording: "play",
  Playing: "dub",
  Overdubbing: "stop",
};

export function createTrackStrip({ index, api }) {
  const color = TRACK_COLORS[index % TRACK_COLORS.length];

  const stateLabel = el("span", { class: "track-state", text: "STOPPED" });

  const ringBase = el("circle", {
    class: "ring-base",
    cx: 50,
    cy: 50,
    r: RING_RADIUS,
  });
  const ringProgress = el("circle", {
    class: "ring-progress",
    cx: 50,
    cy: 50,
    r: RING_RADIUS,
    "stroke-dasharray": RING_CIRCUMFERENCE.toFixed(2),
    "stroke-dashoffset": RING_CIRCUMFERENCE.toFixed(2),
  });
  const ringDot = el("circle", { class: "ring-dot", cx: 50, cy: 6, r: 2.6 });

  const ring = el(
    "svg",
    { class: "ring", viewBox: "0 0 100 100", "aria-hidden": "true" },
    el("circle", { class: "ring-track", cx: 50, cy: 50, r: RING_RADIUS }),
    ringBase,
    ringProgress,
    ringDot,
  );

  const bigButton = el("button", {
    class: "big-btn",
    type: "button",
    text: "REC",
    title: "Record / play / overdub / stop",
    onclick: (event) => {
      event.stopPropagation();
      const current = root.dataset.state || "Stopped";
      api.action(index, NEXT_ACTION[current] ?? "record");
    },
  });

  const ringWrap = el("div", { class: "ring-wrap" }, ring, bigButton);

  const actionButton = (action, label) =>
    el("button", {
      class: "hw-btn",
      type: "button",
      text: label,
      onclick: (event) => {
        event.stopPropagation();
        api.action(index, action);
      },
    });

  const fader = el("input", {
    class: "fader",
    type: "range",
    min: 0,
    max: 100,
    step: 1,
    value: 85,
    "aria-label": `Track ${index + 1} volume`,
    oninput: () => api.setVolume(index, Number(fader.value) / 100),
  });

  const panKnob = el("div", {
    class: "knob",
    role: "slider",
    "aria-label": `Track ${index + 1} pan`,
    tabindex: "0",
  });
  const pan = attachKnob(panKnob, {
    min: -1,
    max: 1,
    value: 0,
    onChange: (value) => api.setPan(index, value),
    format: (value) =>
      Math.abs(value) < 0.02
        ? "C"
        : `${value < 0 ? "L" : "R"}${Math.round(Math.abs(value) * 100)}`,
  });

  const meterFill = el("div", { class: "track-meter-fill" });

  const fxSelect = (slot) => {
    const select = el("select", {
      class: "hw-select",
      "aria-label": `Track ${index + 1} FX ${slot.toUpperCase()}`,
      onchange: () => api.setFx(index, slot, Number(select.value)),
      onclick: (event) => event.stopPropagation(),
    });
    fillSelect(select, FX_KINDS, 0);
    return select;
  };
  const fxA = fxSelect("a");
  const fxB = fxSelect("b");

  const flagButton = (flag, label) =>
    el("button", {
      class: "hw-btn",
      type: "button",
      text: label,
      dataset: { flag },
      onclick: (event) => {
        event.stopPropagation();
        const active = event.currentTarget.classList.toggle("active");
        api.setFlag(index, flag, active);
      },
    });

  const muteButton = flagButton("mute", "MUTE");
  const reverseButton = flagButton("reverse", "REV");
  const oneShotButton = flagButton("oneShot", "1SHOT");

  const root = el(
    "article",
    {
      class: "track-strip",
      dataset: { state: "Stopped", track: String(index) },
      style: { "--track-color": color },
      onclick: () => api.select(index),
    },
    el(
      "div",
      { class: "track-head" },
      el("span", { class: "track-name", text: `TRACK ${index + 1}` }),
      stateLabel,
    ),
    ringWrap,
    el(
      "div",
      { class: "track-actions" },
      actionButton("stop", "STOP"),
      actionButton("clear", "CLEAR"),
      actionButton("undo", "UNDO"),
      actionButton("redo", "REDO"),
    ),
    el(
      "div",
      { class: "track-controls" },
      el(
        "div",
        { class: "fader-col" },
        fader,
        el("span", { class: "knob-caption", text: "VOL" }),
      ),
      el(
        "div",
        { class: "track-meters" },
        panKnob,
        el("span", { class: "knob-caption", text: "PAN" }),
      ),
      el("div", { class: "track-meter", title: "Level" }, meterFill),
    ),
    el("div", { class: "track-selects" }, fxA, fxB),
    el("div", { class: "toggle-row" }, muteButton, reverseButton, oneShotButton),
  );

  function update(track, { selected = false } = {}) {
    root.dataset.state = track.state;
    root.classList.toggle("selected", selected);

    stateLabel.textContent = track.state.toUpperCase();
    bigButton.textContent = BIG_BUTTON_LABEL[track.state] ?? "REC";

    const progress =
      track.state === "Recording" ? track.fill : track.state === "Stopped" ? 0 : track.position;
    const clamped = Math.min(1, Math.max(0, progress || 0));
    ringProgress.setAttribute("stroke-dashoffset", (RING_CIRCUMFERENCE * (1 - clamped)).toFixed(2));
    const angle = clamped * Math.PI * 2;
    ringDot.setAttribute("cx", (50 + RING_RADIUS * Math.cos(angle)).toFixed(2));
    ringDot.setAttribute("cy", (50 + RING_RADIUS * Math.sin(angle)).toFixed(2));
    ringDot.style.opacity = track.state === "Stopped" ? "0.25" : "1";

    if (document.activeElement !== fader) {
      fader.value = String(Math.round(track.volume * 100));
    }
    pan.set(track.pan);

    meterFill.style.height = `${Math.min(100, Math.round(track.peak * 130))}%`;

    if (document.activeElement !== fxA) fxA.value = String(track.fxA);
    if (document.activeElement !== fxB) fxB.value = String(track.fxB);

    muteButton.classList.toggle("active", track.mute);
    reverseButton.classList.toggle("active", track.reverse);
    oneShotButton.classList.toggle("active", track.oneShot);
  }

  return { root, update, elements: { fader, panKnob, bigButton } };
}
