/**
 * RHYTHM panel — internal rhythm generator controls (genre, pattern A-D, beat
 * signature and level).
 */

import { attachKnob, el, fillSelect } from "../lib/dom.js";
import { BEAT_LABELS, GENRE_LABELS } from "../lib/simulator.js";

export function createRhythmPanel({ api }) {
  let state = { enabled: false, genre: 0, pattern: 0, beat: 2, volume: 0.5 };

  const powerButton = el("button", {
    class: "hw-btn",
    type: "button",
    text: "RHYTHM",
    onclick: () => {
      state.enabled = !state.enabled;
      powerButton.classList.toggle("active", state.enabled);
      api.setRhythm(state);
    },
  });

  const genreSelect = el("select", {
    class: "hw-select",
    "aria-label": "Rhythm genre",
    onchange: () => {
      state.genre = Number(genreSelect.value);
      api.setRhythm(state);
    },
  });
  fillSelect(genreSelect, GENRE_LABELS, state.genre);

  const beatSelect = el("select", {
    class: "hw-select",
    "aria-label": "Beat signature",
    onchange: () => {
      state.beat = Number(beatSelect.value);
      api.setRhythm(state);
    },
  });
  fillSelect(beatSelect, BEAT_LABELS, state.beat);

  const patternButtons = ["A", "B", "C", "D"].map((label, index) =>
    el("button", {
      class: `hw-btn small${index === 0 ? " active" : ""}`,
      type: "button",
      text: label,
      onclick: () => {
        state.pattern = index;
        patternButtons.forEach((button, i) => button.classList.toggle("active", i === index));
        api.setRhythm(state);
      },
    }),
  );

  const volumeKnob = el("div", {
    class: "knob",
    role: "slider",
    "aria-label": "Rhythm volume",
    tabindex: "0",
  });
  attachKnob(volumeKnob, {
    min: 0,
    max: 1,
    value: state.volume,
    onChange: (value) => {
      state.volume = value;
      api.setRhythm(state);
    },
    format: (value) => `${Math.round(value * 100)}%`,
  });

  const root = el(
    "div",
    { class: "panel-block" },
    el("div", { class: "panel-title" }, el("span", { text: "RHYTHM" }), el("strong", { text: "GENERATOR" })),
    el("div", { class: "panel-row" }, powerButton),
    el(
      "div",
      { class: "panel-row" },
      el("div", { class: "field", style: { flex: "1" } }, el("span", { class: "field-label", text: "GENRE" }), genreSelect),
      el("div", { class: "field", style: { flex: "1" } }, el("span", { class: "field-label", text: "BEAT" }), beatSelect),
    ),
    el("div", { class: "panel-row" }, el("span", { class: "field-label", text: "PATTERN" }), ...patternButtons),
    el("div", { class: "panel-row" }, volumeKnob, el("span", { class: "knob-caption", text: "LEVEL" })),
  );

  function update(snapshot) {
    const rhythm = snapshot.rhythm;
    const genreIndex = GENRE_LABELS.indexOf(rhythm.genre);
    const beatIndex = BEAT_LABELS.indexOf(rhythm.beat);

    if (genreIndex >= 0 && document.activeElement !== genreSelect) {
      state.genre = genreIndex;
      genreSelect.value = String(genreIndex);
    }
    if (beatIndex >= 0 && document.activeElement !== beatSelect) {
      state.beat = beatIndex;
      beatSelect.value = String(beatIndex);
    }
    const patternIndex = ["A", "B", "C", "D"].indexOf(rhythm.pattern);
    if (patternIndex >= 0) {
      state.pattern = patternIndex;
      patternButtons.forEach((button, index) => button.classList.toggle("active", index === patternIndex));
    }
    state.enabled = rhythm.enabled;
    state.volume = rhythm.volume;
    powerButton.classList.toggle("active", rhythm.enabled);
    volumeKnob.style.setProperty(
      "--angle",
      `${-135 + Math.min(1, Math.max(0, rhythm.volume)) * 270}deg`,
    );
  }

  return { root, update };
}
