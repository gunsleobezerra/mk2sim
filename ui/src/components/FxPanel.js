/**
 * INPUT FX panel — the two input effect slots plus the wet level knob.
 */

import { attachKnob, el, fillSelect } from "../lib/dom.js";
import { FX_KINDS } from "../lib/simulator.js";

export function createInputFxPanel({ api }) {
  const selectA = el("select", {
    class: "hw-select",
    "aria-label": "Input FX A",
    onchange: () => api.setInputFx("a", Number(selectA.value)),
  });
  const selectB = el("select", {
    class: "hw-select",
    "aria-label": "Input FX B",
    onchange: () => api.setInputFx("b", Number(selectB.value)),
  });
  fillSelect(selectA, FX_KINDS, 0);
  fillSelect(selectB, FX_KINDS, 0);

  const levelKnob = el("div", {
    class: "knob",
    role: "slider",
    "aria-label": "Input FX level",
    tabindex: "0",
  });
  attachKnob(levelKnob, {
    min: 0,
    max: 1,
    value: 0.5,
    onChange: (value) => api.setInputFxLevel(value),
    format: (value) => `${Math.round(value * 100)}%`,
  });

  const root = el(
    "div",
    { class: "panel-block" },
    el("div", { class: "panel-title" }, el("span", { text: "INPUT FX" }), el("strong", { text: "A / B" })),
    el("div", { class: "field" }, el("span", { class: "field-label", text: "SLOT A" }), selectA),
    el("div", { class: "field" }, el("span", { class: "field-label", text: "SLOT B" }), selectB),
    el(
      "div",
      { class: "panel-row", style: { marginTop: "10px" } },
      levelKnob,
      el("span", { class: "knob-caption", text: "FX LEVEL" }),
    ),
  );

  return { root, update() {} };
}
