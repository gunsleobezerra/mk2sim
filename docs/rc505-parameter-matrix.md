# Matriz de Parâmetros — RC-505 MkII Clone

Documento de validação da tarefa **QA-02**. Cada parâmetro lista faixa, default, o
atomic que o armazena em `src-tauri/src/state.rs` e o ponto de aplicação no
processamento de áudio ou na camada MIDI.

## 1. Menus do sistema

| Menu | Opções | Default | Armazenamento | Aplicação |
|---|---|---|---|---|
| Input Mode | Instrument / Demo | Instrument (`0`) | `SharedState::input_mode: AtomicU8` | `engine.rs` → `demo_mode` |
| Quantize | Off, 1/16, 1/8, 1/4, 1 Bar, 2 Bars | 1/4 (`3`) | `SharedState::quantize: AtomicU8` | `Track::request` → `clock.next_boundary` |
| Sync | Immediate, Measure, Loop Length | Measure (`1`) | `SharedState::sync_mode: AtomicU8` | `Track::request` → `clock.next_boundary` |
| BPM | 40.0 – 300.0 | 120.0 | `SharedState::bpm: AtomicU32` (bits de f32) | `Clock::configure` por bloco |
| Rhythm Genre | Acoustic, Rock, Electro, Guide | Acoustic (`0`) | `SharedState::rhythm_genre: AtomicU8` | `RhythmEngine::process` |
| Rhythm Pattern | A, B, C, D | A (`0`) | `SharedState::rhythm_pattern: AtomicU8` | `RhythmEngine::process` |
| Rhythm Beat | 2/4, 3/4, 4/4, 5/4, 6/8, 7/8, 9/8, 15/8 | 4/4 | `SharedState::rhythm_beat: AtomicU8` | `RhythmEngine::process` |

## 2. Parâmetros de memória (por track)

`TrackShared` em `state.rs`, consumidos por bloco via `TrackBlockParams` e
aplicados em `audio/track.rs::Track::process`.

| Parâmetro | Faixa | Default | Atomic | Aplicação |
|---|---|---|---|---|
| Volume | 0.0 – 1.0 | 0.85 | `TrackShared::volume` | fader final em `process` |
| Pan | −1.0 (L) – +1.0 (R) | 0.0 | `TrackShared::pan` | pan law **linear**: `gain_l = 1 − max(pan,0)`, `gain_r = 1 + min(pan,0)`; centro = 1.0 |
| Mute | on/off | off | `TrackShared::mute` | multiplica saída por 0 |
| Reverse | on/off | off | `TrackShared::reverse` | leitura `loop_len − 1 − play_pos` |
| 1Shot | on/off | off | `TrackShared::one_shot` | `process` para a track no fim de uma passagem |
| Dub Mode | Overdub, Replace1, Replace2 | Overdub | `TrackShared::dub_mode` | `process`, ramo `Overdubbing` |
| FX A | `EffectKind` 0–15 | 0 (None) | `TrackShared::fx_a` | primeiro slot da cadeia |
| FX B | `EffectKind` 0–15 | 0 (None) | `TrackShared::fx_b` | segundo slot da cadeia |
| FX Level | 0.0 – 1.0 (dry/wet) | 1.0 | `TrackShared::fx_level` | blend `dry*(1−level) + wet*level` |
| EQ Low | −15 … +15 dB | 0.0 | `TrackShared::eq_low` | `ThreeBandEq::process`, shelf de 200 Hz |
| EQ Low-Mid | −15 … +15 dB | 0.0 | `TrackShared::eq_mid` | `ThreeBandEq::process`, banda média |
| EQ High-Mid | −15 … +15 dB | 0.0 | `TrackShared::eq_mid` | compartilha o controle de médios |
| EQ High | −15 … +15 dB | 0.0 | `TrackShared::eq_high` | shelf de 4 kHz |
| Dynamics Comp | on/off | off | `TrackShared::comp` | `Dynamics::process`, threshold 0.35, ratio 0.65 |
| Dynamics NS | on/off | off | `TrackShared::ns` | `Dynamics::process`, gate 0.0015 – 0.0055 |

### Notas de comportamento

- **Dub Mode Replace1** só sobrescreve o buffer quando o input excede
  `INPUT_THRESHOLD = 0.001`; silêncio preserva o material gravado.
- **Dub Mode Replace2** limpa a região de destino na primeira passagem e depois
  grava normalmente.
- **Undo/Redo** guardam a camada de overdub em `undo_layer` / `redo_layer`;
  camadas vazias não geram ponto de undo.
- **EQ** é um esboço de 3 filtros (low shelf, mid, high shelf) mapeado sobre os
  4 controles do painel; `eq_mid` governa Low-Mid e High-Mid.

## 3. Matriz ASSIGN 1–16

Os slots ASSIGN são `CcMapping { cc, channel, track, action }` em
`src-tauri/src/midi/controller.rs`. O mapeamento padrão (`default_mapping()`)
cria 6 CCs de transporte para cada uma das 5 tracks:

| CC | Ação | Track |
|---|---|---|
| 20 | record | canal MIDI = índice da track |
| 21 | play | idem |
| 22 | dub | idem |
| 23 | stop | idem |
| 24 | clear | idem |
| 25 | undo | idem |

Controles contínuos:

| CC | Ação | Observação |
|---|---|---|
| 7 | Volume | normalizado 0–127 → 0.0–1.0 |
| 10 | Pan | normalizado 0–127 → −1.0–+1.0 |

Notas:

| Nota | Ação |
|---|---|
| 36–40 | Play da track `nota − 36` |
| 41–45 | Record da track `nota − 41` |

Clock e transporte: `0xF8` clock, `0xFA` start, `0xFB` continue, `0xFC` stop.

Slots ASSIGN 1–16 adicionais são registrados com `MidiHub::set_mapping`, que
substitui qualquer mapeamento existente para o mesmo par `(cc, channel)`.

## 4. Cobertura de teste correspondente

| Item validado | Teste |
|---|---|
| Pan / ganho unitário no centro | `audio::track::tests::recording_sets_loop_length_and_plays_back` |
| Dub Mode Overdub + Undo/Redo | `audio::track::tests::overdub_adds_and_undo_subtracts` |
| Dub Mode Replace1 | `audio::track::tests::replace1_only_overwrites_when_input_present` |
| Reverse | `audio::track::tests::reverse_plays_buffer_backwards` |
| 1Shot | `audio::track::tests::one_shot_stops_after_a_pass` |
| Quantize / Sync | `audio::track::tests::quantized_request_is_deferred`, `audio::clock::tests::measure_boundary_waits_for_grid` |
| EQ / Dynamics | `dsp::effects::tests::eq_and_dynamics_are_finite` |
| ASSIGN / CC / notas | `midi::controller::tests::*` |
