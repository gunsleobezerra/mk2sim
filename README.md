# RC-505 MkII Clone

Réplica funcional do pedal Boss RC-505 MkII: looper de 5 tracks, engine de áudio
Rust em tempo real, MIDI, efeitos DSP e interface web que imita o hardware.

- **Backend:** Rust + Tauri 2 (`src-tauri/`)
- **Frontend:** HTML/CSS/JS puro, ES modules, sem bundler (`ui/`)
- **Spec de origem:** `agent-tasks-rc505.json` (5 agentes, 16 tarefas)

## Status

| Agente | Tarefas | Status |
|---|---|---|
| `agent_01_architect` | ARCH-01, ARCH-02 | ✅ |
| `agent_02_audio_dsp` | AUDIO-01 … AUDIO-05 | ✅ |
| `agent_03_midi_plugins` | MIDI-01, MIDI-02 | ✅ (plugin = passthrough) |
| `agent_04_frontend_ui` | UI-01 … UI-04 | ✅ |
| `agent_05_qa_validation` | QA-01, QA-02 | ✅ 36/36 testes |

## Como rodar

### Testes

```bash
cd src-tauri
cargo test --lib
```

### App desktop (Tauri)

```bash
cargo install tauri-cli --version "^2"   # uma vez
cd src-tauri
cargo tauri dev
```

Sem a CLI do Tauri também funciona compilando direto:

```bash
cd src-tauri
cargo run
```

O app abre com `frontendDist: "../ui"`, janela `main` de 1360×860 (mínimo
1080×700). Se nenhum dispositivo de áudio for inicializado, o engine cai para um
renderer por software e continua tocando (útil para desenvolvimento e CI).

### Preview no navegador

Abra `ui/index.html` direto no browser. `ui/src/lib/bridge.js` detecta a ausência
de `window.__TAURI__` e usa `ui/src/lib/simulator.js`, uma máquina de estados
completa em JavaScript que responde aos mesmos 27 comandos do backend.

## Arquitetura

```
┌──────────────────────────── ui/ (JS puro) ─────────────────────────────┐
│ index.html → app.js → components/ (TrackStrip, LCDDisplay, FxPanel, …) │
│                       ↘ lib/bridge.js ──┬── Tauri invoke/listen        │
│                                         └── lib/simulator.js (browser) │
└────────────────────────────────┬───────────────────────────────────────┘
                                 │ 27 comandos + evento rc505://state (60 fps)
┌────────────────────────────────┴───────────────────────────────────────┐
│ src-tauri/src/lib.rs — AppState, Snapshot, emitter de 16 ms            │
│                                                                        │
│  state.rs  atomics (f32 em bits, enums u8) + canal crossbeam (cap 512) │
│                                                                        │
│  audio/engine.rs   CPAL | ringbuf | fallback software                  │
│  audio/track.rs    5 tracks: Rec/Play/Dub/Stop, undo/redo, reverse     │
│  audio/clock.rs    BPM, loop-to-tempo, quantize, sync                  │
│  audio/plugin_host.rs  varredura VST3/AU + libloading                  │
│  dsp/effects.rs    16 efeitos + EQ 3 bandas + Dynamics                 │
│  dsp/rhythm.rs     4 gêneros, padrões A–D, 8 assinaturas               │
│  midi/controller.rs  midir, CC/Note/Clock, ASSIGN                      │
└────────────────────────────────────────────────────────────────────────┘
```

### Modelo de concorrência

O audio thread nunca bloqueia. Parâmetros chegam como atomics lidos uma vez por
bloco (`TrackBlockParams::snapshot`), e transições de estado viajam por um canal
`crossbeam` limitado. A UI lê um `Snapshot` serializado a cada 16 ms.

### Pan law

Linear: `gain_l = 1 − max(pan, 0)`, `gain_r = 1 + min(pan, 0)`. Centro = ganho
unitário nos dois canais, sem atenuação de −3 dB.

### Monitoração direta

O sinal de entrada pós-Input-FX entra na soma master, como no hardware — o
instrumento é sempre audível, inclusive no modo Demo (tom de teste de 220 Hz +
331 Hz quando não há dispositivo de entrada).

## Limitações conhecidas

- **Plugins VST3/AU:** a varredura e o carregamento dinâmico (`libloading`)
  funcionam e são testados, mas o processamento é **passthrough**. O seam existe
  em `audio/plugin_host.rs` para plugar um host real.
- **EQ de 4 bandas:** implementado como low shelf + mid + high shelf; os dois
  controles de médio compartilham `eq_mid`.
- **Gravação:** máximo de 20 s por track (`MAX_LOOP_SECONDS`), buffers
  pré-alocados.

## Agentes e skills

Definições em `.github/agents/`:

- `agent_01_architect.agent.md`
- `agent_02_audio_dsp.agent.md`
- `agent_03_midi_plugins.agent.md`
- `agent_04_frontend_ui.agent.md`
- `agent_05_qa_validation.agent.md`

Skills em `.github/skills/`: `rc505-tauri-ipc`, `rc505-audio-dsp`,
`rc505-midi-plugins`, `rc505-hardware-ui`, `rc505-qa-validation`.

## Documentação

- `docs/rc505-parameter-matrix.md` — matriz de parâmetros e ASSIGN (QA-02).
- `graphify-out/GRAPH_REPORT.md` — visão arquitetural gerada pelo Graphify.

## Knowledge graph

```bash
graphify query "onde o pan é aplicado"
graphify affected "Track::process" --depth 2
graphify update .        # após criar/mover/remover módulos
```
