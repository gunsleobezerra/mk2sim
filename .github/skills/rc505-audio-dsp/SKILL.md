---
name: rc505-audio-dsp
description: 'Padrões de DSP e áudio em tempo real do RC-505 MkII Clone: implementar um efeito novo, mexer no looper de tracks, ajustar clock/quantize ou o rhythm guide. Use para qualquer mudança em src-tauri/src/audio ou src-tauri/src/dsp.'
---

# Skill — Áudio & DSP

## Quando usar
- Criar um efeito em `dsp/effects.rs`.
- Alterar comportamento de record/overdub/undo em `audio/track.rs`.
- Mexer em BPM, quantize ou sync em `audio/clock.rs`.
- Alterar o rhythm guide em `dsp/rhythm.rs`.

## Adicionar um efeito
1. Variante nova em `EffectKind` + entrada em `EffectKind::ALL` (hoje 16).
2. `from_u8` / `as_u8` e `label()`.
3. Struct com `DspEffect` (`prepare(sr)`, `process(l, r, sr) -> (f32, f32)`, `reset()`).
4. Braço no `match` de `EffectSlot::process`.
5. `cargo test --lib` — `every_effect_stays_finite` e `effect_kinds_round_trip` cobrem o novo caso automaticamente.
6. Espelhar o rótulo em `ui/src/lib/simulator.js` (`FX_KINDS`).

## Regras de tempo real
- Zero alocação, zero lock, zero I/O dentro de `process`.
- Coeficientes recalculados só quando o parâmetro muda, nunca por amostra.
- Saturação final via `soft_clip` (`x/(1+|x|)` sobre ±3).

## Looper (`track.rs`)
- Estados: `Stopped`, `Recording`, `Playing`, `Overdubbing`.
- `request()` agenda; `tick()` aplica no boundary do clock. `Quantize::Off` + `SyncMode::Immediate` = aplicação no próximo frame.
- Pan law linear: `gain_l = 1 - max(pan,0)`, `gain_r = 1 + min(pan,0)`. Centro = 1.0.
- `DubMode::Replace1` só sobrescreve quando o input passa de `INPUT_THRESHOLD` (0.001).
- Undo guarda a camada de overdub só se ela tiver conteúdo.

## Validação
```bash
cd src-tauri
cargo test --lib
cargo test --lib audio::track -- --nocapture
```
