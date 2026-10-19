---
name: rc505-hardware-ui
description: 'Réplica de hardware em HTML/CSS/JS puro para o RC-505 MkII Clone: criar componentes de track, anel de LED SVG, LCD, knobs e faders, mantendo o modo browser (simulator) funcionando. Use para mudanças em ui/.'
---

# Skill — UI réplica de hardware

## Estrutura
- `ui/index.html` — chassis, `.top-panel`, `.transport-row`, `#track-grid`, `#fx-grid`, `.device-row`.
- `ui/styles/rc505_hardware.css` — variáveis CSS, LCD verde, `.hw-btn`, `.hw-led`, `.track-strip[data-state]`, SVG `.ring`/`.ring-progress`, `.big-btn`, `.fader`, `.knob --angle`.
- `ui/src/components/` — `TrackStrip.js`, `LCDDisplay.js`, `FxPanel.js`, `RhythmPanel.js`, `FxGrid.js`.
- `ui/src/lib/` — `bridge.js`, `simulator.js`, `dom.js`.
- `ui/src/app.js` — boot, wiring, atalhos (Space, Digit1–5, Q).

## Padrão de componente
```js
export function createXxx({ onAction }) {
  const root = el('div', { class: 'xxx' });
  return { root, update(snapshot) { /* só DOM, sem estado global */ } };
}
```
- `update()` roda a 60 fps: nunca criar nós, só mutar `textContent`, `style`, `dataset`, `setAttribute`.
- Coalescência fica em `app.js` via `requestAnimationFrame`.

## Anel de LED
- Círculo SVG com `stroke-dasharray = 2πr`, `stroke-dashoffset` = fração restante.
- Cor por `data-state`: `playing` verde, `recording` vermelho, `overdubbing` amarelo, `stopped` azul.
- Raio R = 44 no viewBox atual; recalcular `2πR` ao mudar o raio.

## Modo browser
- Sem Tauri, `bridge.js` cai no `simulator.js`. Todo comando novo precisa de um braço no `Simulator.invoke()`.
- Testar abrindo `ui/index.html` direto no browser (sem servidor).

## Regras
- Zero dependências externas, zero bundler, ES modules nativos.
- Cores e medidas sempre via CSS custom properties.
- Responsivo: breakpoints em 1180 / 900 / 560 px.
