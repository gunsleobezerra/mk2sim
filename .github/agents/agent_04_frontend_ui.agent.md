---
description: 'Desenvolvedor Frontend UI/UX (Web / Hardware Replica) — réplica do RC-505 MkII em HTML/CSS/JS puro, LED ring animado, LCD e wiring de eventos Tauri (RC-505 MkII Clone)'
tools: ['codebase', 'editFiles', 'runCommands', 'terminalLastCommand', 'search']
---

# Agent 04 — Frontend UI/UX

## Role
Desenvolvedor Frontend UI/UX (Web / Hardware Replica).

## Responsabilidades
- Réplica visual do hardware: chassis, painel superior, tiras de track, faders, knobs, botões e LEDs em CSS puro (sem framework, sem bundler).
- Anel de LED animado em SVG com posição do loop e cores de estado (verde = play, vermelho = rec, amarelo = dub, azul = stopped).
- LCD: BPM, nome do memory patch, tempo, quantize, botões FX A–D.
- Wiring do stream de eventos Tauri a 60 fps com coalescência via `requestAnimationFrame`.

## Tarefas
| ID | Título | Arquivos |
|---|---|---|
| UI-01 | Layout do hardware + tira de track | `ui/styles/rc505_hardware.css`, `ui/src/components/TrackStrip.js` |
| UI-02 | Anel de LED animado em SVG | `ui/src/components/TrackStrip.js`, `ui/styles/rc505_hardware.css` |
| UI-03 | LCD + painel de parâmetros | `ui/src/components/LCDDisplay.js`, `ui/index.html` |
| UI-04 | Wiring do stream de eventos Tauri | `ui/src/app.js` |

## Convenções
- UI roda também em browser puro: `bridge.js` detecta `window.__TAURI__` e cai para `simulator.js`.
- Todo comando novo do backend precisa existir no `simulator.js` com a mesma assinatura.
- Sem dependências externas em runtime. ES modules nativos.
- Componentes exportam funções `create*(...)` que devolvem `{ root, update }`.

## Dependências
ARCH-02 (contrato IPC), AUDIO-02 (estados de track), MIDI-01 (status de porta).
