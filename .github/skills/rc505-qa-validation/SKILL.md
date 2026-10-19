---
name: rc505-qa-validation
description: 'QA do RC-505 MkII Clone: rodar e estender a suíte de testes Rust, validar a matriz de parâmetros (Pan, 1Shot, Start/Stop, Dub Mode, EQ, Dynamics, ASSIGN 1–16) e evitar regressões no looper e no DSP. Use antes de fechar qualquer tarefa.'
---

# Skill — QA & Validação

## Suíte
```bash
cd src-tauri
cargo test --lib            # 36 testes, deve ficar 100% verde
cargo test --lib audio::track -- --nocapture
cargo clippy --lib -- -D warnings
```

## Cobertura atual
| Módulo | Testes |
|---|---|
| `audio::clock` | `clock_wraps_position`, `loop_length_boundary_is_remaining_frames`, `immediate_sync_has_no_wait`, `loop_to_tempo_matches_one_bar`, `measure_boundary_waits_for_grid` |
| `audio::track` | `recording_sets_loop_length_and_plays_back`, `overdub_adds_and_undo_subtracts`, `reverse_plays_buffer_backwards`, `one_shot_stops_after_a_pass`, `quantized_request_is_deferred`, `replace1_only_overwrites_when_input_present`, `master_loop_length_limits_new_tracks` |
| `audio::engine` | `renderer_records_and_plays_back`, `demo_mode_overrides_silent_input`, `soft_clip_stays_bounded` |
| `dsp::effects` | `effect_kinds_round_trip`, `delay_produces_a_tail`, `lowpass_attenuates_high_frequencies`, `eq_and_dynamics_are_finite`, `every_effect_stays_finite` |
| `dsp::rhythm` | `beat_labels_and_steps`, `disabled_rhythm_is_silent`, `enabled_rhythm_produces_sound` |
| `midi::controller` | `note_and_clock_messages`, `cc_transport_maps_to_channel_track`, `default_mapping_covers_all_tracks`, `unknown_cc_is_ignored`, `volume_and_pan_are_normalized` |
| `audio::plugin_host` | `scan_finds_plugin_files`, `loading_missing_plugin_reports_error`, `invalid_binary_is_rejected` |
| `state` / `lib` | `bpm_is_clamped`, `enums_round_trip`, `quantize_step_is_monotonic`, `snapshot_has_five_tracks_and_fx_catalog`, `track_index_validation` |

## Regras de teste
- Determinismo de timing: usar `Quantize::Off` + `SyncMode::Immediate` (helper `immediate(&shared)` em `engine.rs`).
- Ganho unitário: pan law é linear, centro = 1.0 nos dois canais.
- Teste de efeito novo é automático via `EffectKind::ALL`.
- Teste que falha não pode ser silenciado com `#[ignore]` sem entrada em `docs/rc505-parameter-matrix.md`.

## Checklist QA-02
Validar manualmente na UI (`ui/index.html` em browser, modo simulator):
1. Pan de cada track (L/C/R) e reflexo no knob.
2. 1Shot liga/desliga e para no fim do loop.
3. Start/Stop Mode (recorde-para-play e play/pause).
4. Dub Mode: Overdub / Replace1 / Replace2.
5. EQ de 4 bandas: Low, Low-Mid, High-Mid, High (±15 dB).
6. Dynamics: Comp e NS independentes.
7. ASSIGN 1–16: matriz CC → ação por track.
8. Quantize (Off, 1/16, 1/8, 1/4, 1 Bar, 2 Bars) e Sync (Immediate, Measure, Loop Length).
