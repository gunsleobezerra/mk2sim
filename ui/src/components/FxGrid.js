/**
 * FX grid — effect assignment for the selected track, plus EQ, dynamics and dub
 * mode controls (the ASSIGN section of the hardware).
 */

import { attachKnob, el, fillSelect, formatSigned } from "../lib/dom.js";
import { DUB_MODE_LABELS, FX_KINDS } from "../lib/simulator.js";
import { TRACK_COLORS } from "./TrackStrip.js";

export function createFxGrid({ api }) {
  let selected = 0;
  let slot = "a";
  let eq = { low: 0, mid: 0, high: 0 };

  const trackLabel = el("strong", { text: "TRACK 1" });

  const dubSelect = el("select", {
    class: "hw-select",
    "aria-label": "Dub mode",
    onchange: () => api.setDubMode(selected, Number(dubSelect.value)),
  });
  fillSelect(dubSelect, DUB_MODE_LABELS, 0);

  const slotButtons = ["a", "b"].map((value) =>
    el("button", {
      class: `hw-btn small${value === "a" ? " active" : ""}`,
      type: "button",
      text: `FX ${value.toUpperCase()}`,
      onclick: () => {
        slot = value;
        slotButtons.forEach((button, index) =>
          button.classList.toggle("active", ["a", "b"][index] === slot),
        );
        if (lastTrack) paintFx(lastTrack);
      },
    }),
  );

  const effectButtons = FX_KINDS.map((label, kind) =>
    el("button", {
      class: "fx-btn",
      type: "button",
      text: label,
      dataset: { kind: String(kind) },
      onclick: () => api.setFx(selected, slot, kind),
    }),
  );

  const eqKnobs = {};
  const eqKnob = (key, label) => {
    const element = el("div", {
      class: "knob",
      role: "slider",
      "aria-label": `Track EQ ${label}`,
      tabindex: "0",
    });
    const controller = attachKnob(element, {
      min: -15,
      max: 15,
      value: 0,
      step: 0.05,
      onChange: (value) => {
        eq[key] = value;
        api.setEq(selected, eq.low, eq.mid, eq.high);
      },
      format: (value) => `${formatSigned(value)} dB`,
    });
    eqKnobs[key] = controller;
    return el(
      "div",
      { class: "field", style: { alignItems: "center" } },
      element,
      el("span", { class: "knob-caption", text: label }),
    );
  };

  const fxLevelKnob = el("div", {
    class: "knob",
    role: "slider",
    "aria-label": "Track FX level",
    tabindex: "0",
  });
  const fxLevel = attachKnob(fxLevelKnob, {
    min: 0,
    max: 1,
    value: 0.5,
    onChange: (value) => api.setTrackFxLevel(selected, value),
    format: (value) => `${Math.round(value * 100)}%`,
  });

  const compButton = el("button", {
    class: "hw-btn small",
    type: "button",
    text: "COMP",
    onclick: () => api.setFlag(selected, "comp", !compButton.classList.contains("active")),
  });
  const nsButton = el("button", {
    class: "hw-btn small",
    type: "button",
    text: "NS",
    onclick: () => api.setFlag(selected, "ns", !nsButton.classList.contains("active")),
  });

  const root = el(
    "section",
    { class: "fx-grid" },
    el(
      "div",
      { class: "fx-assign-head" },
      el("span", { class: "field-label", text: "TRACK FX · EDIT" }),
      trackLabel,
      el(
        "div",
        { class: "dub-mode" },
        el("span", { class: "field-label", text: "DUB MODE" }),
        dubSelect,
      ),
    ),
    el("div", { class: "fx-bank" }, ...effectButtons),
    el(
      "div",
      { class: "fx-params" },
      ...slotButtons,
      el(
        "div",
        { class: "field", style: { alignItems: "center" } },
        fxLevelKnob,
        el("span", { class: "knob-caption", text: "FX LVL" }),
      ),
      el(
        "div",
        { class: "eq-group" },
        eqKnob("low", "LOW"),
        eqKnob("mid", "MID"),
        eqKnob("high", "HIGH"),
      ),
      el("div", { class: "toggle-row", style: { flexDirection: "column" } }, compButton, nsButton),
    ),
  );

  let lastTrack = null;

  function paintFx(track) {
    const active = slot === "a" ? track.fxA : track.fxB;
    effectButtons.forEach((button, kind) => button.classList.toggle("active", kind === active));
  }

  function update(snapshot, selectedIndex = selected) {
    const track = snapshot.tracks[selectedIndex];
    if (!track) return;

    if (selectedIndex !== selected) {
      selected = selectedIndex;
      eq = { low: track.eqLow, mid: track.eqMid, high: track.eqHigh };
    }
    lastTrack = track;

    trackLabel.textContent = `TRACK ${track.id}`;
    trackLabel.style.color = TRACK_COLORS[(track.id - 1) % TRACK_COLORS.length];

    if (document.activeElement !== dubSelect) {
      const index = DUB_MODE_LABELS.indexOf(track.dubMode);
      if (index >= 0) dubSelect.value = String(index);
    }

    paintFx(track);

    fxLevel.set(track.fxLevel);
    eqKnobs.low.set(track.eqLow);
    eqKnobs.mid.set(track.eqMid);
    eqKnobs.high.set(track.eqHigh);

    compButton.classList.toggle("active", track.comp);
    nsButton.classList.toggle("active", track.ns);
  }

  return { root, update };
}
