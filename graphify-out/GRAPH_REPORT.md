# Graph Report - mk2sim  (2026-10-19)

## Corpus Check
- 43 files · ~24,511 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 574 nodes · 1149 edges · 43 communities (31 shown, 6 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 12 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- app.js
- lib.rs
- SharedState
- Track
- engine.rs
- PluginHost
- MidiHub
- RhythmEngine
- DelayLine
- Self
- tauri.conf.json
- clock.rs
- DspEffect
- Simulator
- Reverb
- effects.rs
- PitchShift
- EffectSlot
- default.json
- Phaser
- Slicer
- rc505-clone
- RC-505 MkII Clone
- Agent 05 — QA, Testes & Validação
- Matriz de Parâmetros — RC-505 MkII Clone
- Agent 01 — Arquiteto / Lead Developer
- Agent 02 — Áudio Digital & DSP
- Agent 03 — MIDI & Plugins
- Agent 04 — Frontend UI/UX
- Skill — Áudio & DSP
- Skill — UI réplica de hardware
- Graphify — Instruções para o Agente
- Skill — MIDI & Plugins
- Skill — QA & Validação
- Skill — Tauri IPC do RC-505
- AGENTS.md
- copilot-instructions.md

## God Nodes (most connected - your core abstractions)
1. `AppState` - 35 edges
2. `Track` - 25 edges
3. `SharedState` - 22 edges
4. `MidiHub` - 20 edges
5. `Clock` - 19 edges
6. `DspEffect` - 19 edges
7. `TrackCommand` - 19 edges
8. `PluginHost` - 18 edges
9. `el()` - 16 edges
10. `RhythmEngine` - 14 edges

## Surprising Connections (you probably didn't know these)
- `build_snapshot()` --calls--> `bits_to_f32()`  [INFERRED]
  src-tauri/src/lib.rs → src-tauri/src/state.rs
- `dispatch()` --calls--> `f32_to_bits()`  [INFERRED]
  src-tauri/src/midi/controller.rs → src-tauri/src/state.rs
- `Renderer` --references--> `Clock`  [EXTRACTED]
  src-tauri/src/audio/engine.rs → src-tauri/src/audio/clock.rs
- `AppState` --references--> `TapTempo`  [EXTRACTED]
  src-tauri/src/lib.rs → src-tauri/src/audio/clock.rs
- `AppState` --references--> `EngineHandle`  [EXTRACTED]
  src-tauri/src/lib.rs → src-tauri/src/audio/engine.rs

## Import Cycles
- None detected.

## Communities (43 total, 6 thin omitted)

### Community 0 - "app.js"
Cohesion: 0.07
Nodes (54): api, backendLabel, bpmInput, cpuMeter, engineDetail, flush(), fxGrid, guard() (+46 more)

### Community 1 - "lib.rs"
Cohesion: 0.17
Nodes (43): AppState, build_snapshot(), get_midi_mapping(), get_plugin_dirs(), get_state(), list_midi_ports(), list_plugins(), load_plugin() (+35 more)

### Community 2 - "SharedState"
Cohesion: 0.08
Nodes (20): AtomicU32, AtomicU64, AtomicU8, measure_boundary_waits_for_grid(), bits_to_f32(), bpm_is_clamped(), DubMode, f32_to_bits() (+12 more)

### Community 3 - "Track"
Cohesion: 0.17
Nodes (17): Clock, master_loop_length_limits_new_tracks(), one_shot_stops_after_a_pass(), overdub_adds_and_undo_subtracts(), params(), quantized_request_is_deferred(), record_constant(), recording_sets_loop_length_and_plays_back() (+9 more)

### Community 4 - "engine.rs"
Cohesion: 0.15
Nodes (26): Device, HeapCons, HeapProd, Receiver, build_input(), build_input_stream(), build_output(), demo_mode_overrides_silent_input() (+18 more)

### Community 5 - "PluginHost"
Cohesion: 0.15
Nodes (22): Library, Path, PathBuf, default_dirs(), detect_format(), invalid_binary_is_rejected(), loading_missing_plugin_reports_error(), plugin_name() (+14 more)

### Community 6 - "MidiHub"
Cohesion: 0.15
Nodes (22): cc_transport_maps_to_channel_track(), CcMapping, default_mapping(), default_mapping_covers_all_tracks(), dispatch(), MidiEvent, MidiHub, MidiPort (+14 more)

### Community 7 - "RhythmEngine"
Cohesion: 0.12
Nodes (11): Beat, disabled_rhythm_is_silent(), enabled_rhythm_produces_sound(), Genre, RhythmEngine, Default, Self, Vec (+3 more)

### Community 8 - "DelayLine"
Cohesion: 0.10
Nodes (6): Chorus, Delay, DelayLine, Flanger, Lofi, ReverseDelay

### Community 9 - "Self"
Cohesion: 0.19
Nodes (8): delay_produces_a_tail(), Dynamics, eq_and_dynamics_are_finite(), every_effect_stays_finite(), lowpass_attenuates_high_frequencies(), Default, Self, ThreeBandEq

### Community 10 - "tauri.conf.json"
Cohesion: 0.12
Nodes (15): app, security, windows, withGlobalTauri, build, frontendDist, bundle, active (+7 more)

### Community 11 - "clock.rs"
Cohesion: 0.19
Nodes (10): Instant, bpm_from_loop(), clock_wraps_position(), immediate_sync_has_no_wait(), loop_length_boundary_is_remaining_frames(), loop_to_tempo_matches_one_bar(), Option, Self (+2 more)

### Community 12 - "DspEffect"
Cohesion: 0.13
Nodes (5): Send, BeatRepeat, DspEffect, Passthrough, Tremolo

### Community 13 - "Simulator"
Cohesion: 0.27
Nodes (3): clamp(), createTrack(), simulator

### Community 14 - "Reverb"
Cohesion: 0.18
Nodes (4): AllPass, Comb, Reverb, Vec

### Community 17 - "EffectSlot"
Cohesion: 0.46
Nodes (4): Box, build(), EffectKind, EffectSlot

### Community 18 - "default.json"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 27 - "RC-505 MkII Clone"
Cohesion: 0.13
Nodes (14): Agentes e skills, App desktop (Tauri), Arquitetura, Como rodar, Documentação, Knowledge graph, Limitações conhecidas, Modelo de concorrência (+6 more)

### Community 28 - "Agent 05 — QA, Testes & Validação"
Cohesion: 0.25
Nodes (7): Agent 05 — QA, Testes & Validação, Convenções, Dependências, Responsabilidades, Role, Status atual, Tarefas

### Community 29 - "Matriz de Parâmetros — RC-505 MkII Clone"
Cohesion: 0.29
Nodes (6): 1. Menus do sistema, 2. Parâmetros de memória (por track), 3. Matriz ASSIGN 1–16, 4. Cobertura de teste correspondente, Matriz de Parâmetros — RC-505 MkII Clone, Notas de comportamento

### Community 30 - "Agent 01 — Arquiteto / Lead Developer"
Cohesion: 0.29
Nodes (6): Agent 01 — Arquiteto / Lead Developer, Convenções, Dependências, Responsabilidades, Role, Tarefas

### Community 31 - "Agent 02 — Áudio Digital & DSP"
Cohesion: 0.29
Nodes (6): Agent 02 — Áudio Digital & DSP, Convenções, Dependências, Responsabilidades, Role, Tarefas

### Community 32 - "Agent 03 — MIDI & Plugins"
Cohesion: 0.29
Nodes (6): Agent 03 — MIDI & Plugins, Convenções, Dependências, Responsabilidades, Role, Tarefas

### Community 33 - "Agent 04 — Frontend UI/UX"
Cohesion: 0.29
Nodes (6): Agent 04 — Frontend UI/UX, Convenções, Dependências, Responsabilidades, Role, Tarefas

### Community 34 - "Skill — Áudio & DSP"
Cohesion: 0.29
Nodes (6): Adicionar um efeito, Looper (`track.rs`), Quando usar, Regras de tempo real, Skill — Áudio & DSP, Validação

### Community 35 - "Skill — UI réplica de hardware"
Cohesion: 0.29
Nodes (6): Anel de LED, Estrutura, Modo browser, Padrão de componente, Regras, Skill — UI réplica de hardware

### Community 36 - "Graphify — Instruções para o Agente"
Cohesion: 0.33
Nodes (5): Artefatos, Comandos, Fluxo obrigatório, Graphify — Instruções para o Agente, Regras específicas deste repositório

### Community 37 - "Skill — MIDI & Plugins"
Cohesion: 0.33
Nodes (5): Adicionar um CC novo, Comandos, Mapeamento MIDI, Plugins, Skill — MIDI & Plugins

### Community 38 - "Skill — QA & Validação"
Cohesion: 0.33
Nodes (5): Checklist QA-02, Cobertura atual, Regras de teste, Skill — QA & Validação, Suíte

### Community 39 - "Skill — Tauri IPC do RC-505"
Cohesion: 0.33
Nodes (5): Arquivos, Fluxo obrigatório, Quando usar, Regras, Skill — Tauri IPC do RC-505

### Community 40 - "AGENTS.md"
Cohesion: 0.40
Nodes (3): Comandos, Estrutura, Graphify (knowledge graph do repositório)

## Knowledge Gaps
- **109 isolated node(s):** `rc505-clone`, `$schema`, `identifier`, `description`, `windows` (+104 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 208 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **6 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Track` connect `Track` to `Self`, `SharedState`, `engine.rs`, `EffectSlot`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **Why does `TrackCommand` connect `engine.rs` to `SharedState`, `Track`, `MidiHub`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Why does `AppState` connect `lib.rs` to `clock.rs`, `engine.rs`, `PluginHost`, `MidiHub`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **What connects `rc505-clone`, `$schema`, `identifier` to the rest of the system?**
  _109 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `app.js` be split into smaller, more focused modules?**
  _Cohesion score 0.06634615384615385 - nodes in this community are weakly interconnected._
- **Should `SharedState` be split into smaller, more focused modules?**
  _Cohesion score 0.07610993657505286 - nodes in this community are weakly interconnected._
- **Should `RhythmEngine` be split into smaller, more focused modules?**
  _Cohesion score 0.1225071225071225 - nodes in this community are weakly interconnected._