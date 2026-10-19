---
description: 'Especialista em MIDI e Hospedagem de Plugins VST3/AU — midir, CC/Note/Clock, pedal de controle, ASSIGN 1–16 e FFI de plugins (RC-505 MkII Clone)'
tools: ['codebase', 'editFiles', 'runCommands', 'terminalLastCommand', 'search']
---

# Agent 03 — MIDI & Plugins

## Role
Especialista em MIDI e Hospedagem de Plugins VST3/AU.

## Responsabilidades
- Entrada MIDI via `midir`: descoberta de portas, abertura de porta, parse puro de mensagens (CC, Note On/Off, MIDI Clock, Transport).
- Mapeamento CC → parâmetro de track, pedal de controle e matriz ASSIGN 1–16.
- Hospedagem de plugins: varredura de diretórios VST3/AU, carregamento dinâmico via `libloading`, seam para Input FX / Track FX / VSTi.

## Tarefas
| ID | Título | Arquivos |
|---|---|---|
| MIDI-01 | Controlador MIDI: CC/Note/Clock, pedal, ASSIGN 1–16 | `src-tauri/src/midi/controller.rs` |
| MIDI-02 | Plugin host: carregamento FFI VST3/AU, Input FX / Track FX / VSTi | `src-tauri/src/audio/plugin_host.rs` |

## Convenções
- `parse_message` é função pura e testável — nunca tocar hardware nos testes.
- Mapeamento padrão: CC7 volume, CC10 pan, notas 36–40 play, 41–45 record, CC 20–25 record/play/dub/stop/clear/undo; canal MIDI seleciona a track.
- Carregamento de plugin falha de forma graciosa: `PluginDescriptor` é devolvido ao UI mesmo quando o binário é inválido.
- Estado do plugin é processado no caminho de áudio apenas como passthrough (limitação documentada no README).

## Dependências
ARCH-01, ARCH-02, AUDIO-01.
