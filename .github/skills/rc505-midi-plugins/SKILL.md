---
name: rc505-midi-plugins
description: 'MIDI e plugins do RC-505 MkII Clone: mapear CC/Note/Clock, criar mappings ASSIGN, testar parse de mensagens e carregar VST3/AU via libloading. Use para mudanças em midi/controller.rs ou audio/plugin_host.rs.'
---

# Skill — MIDI & Plugins

## Mapeamento MIDI
- `parse_message(bytes, sample_rate)` é puro: use-o nos testes, sem abrir porta.
- Mapeamento padrão em `MidiHub::default_mapping()`:
  - CC7 → volume da track do canal; CC10 → pan.
  - Notas 36–40 → play; 41–45 → record.
  - CC 20..25 → record / play / dub / stop / clear / undo.
  - `0xF8` clock, `0xFA` start, `0xFB` continue, `0xFC` stop.
- Canal MIDI (0–15) seleciona a track; canal 15 = global quando aplicável.

## Adicionar um CC novo
1. `CcMapping { cc, channel: Option<u8>, track: usize, action: TrackCommand }`.
2. Registrar em `default_mapping()` ou via `set_mapping`.
3. Teste em `midi::controller::tests` usando `parse_message`.
4. Se o CC expõe parâmetro novo de track, atualizar `docs/rc505-parameter-matrix.md`.

## Plugins
- `scan_plugin_dirs()` varre VST3/AU e devolve `PluginDescriptor`.
- `load()` usa `libloading`; binário inválido deve retornar `Err`, nunca panicar.
- Processamento atual é passthrough (seam para Input FX / Track FX / VSTi). Documentado no README como limitação.

## Comandos
```bash
cd src-tauri
cargo test --lib midi::
cargo test --lib plugin_host::
```
