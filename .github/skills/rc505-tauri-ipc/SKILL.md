---
name: rc505-tauri-ipc
description: 'Como estender a camada IPC do RC-505 MkII Clone: adicionar/alterar comandos Tauri em lib.rs e espelhar em bridge.js + simulator.js sem quebrar o stream de 60 fps. Use quando precisar criar um comando, evento ou campo de snapshot novo.'
---

# Skill — Tauri IPC do RC-505

## Quando usar
- Adicionar ou renomear um `#[tauri::command]`.
- Adicionar campo ao `Snapshot` consumido pela UI.
- Alterar o stream `rc505://state`.

## Fluxo obrigatório
1. `graphify query "tauri command snapshot bridge simulator"` para achar consumidores atuais.
2. Editar `src-tauri/src/lib.rs`:
   - `#[tauri::command] fn nome(state: State<AppState>, ...) -> Result<T, String>`
   - registrar em `tauri::generate_handler![...]` no `run()`.
   - se expõe dado novo, adicionar em `Snapshot` / `TrackSnapshot` com `#[serde(rename_all = "camelCase")]`.
3. Espelhar no frontend:
   - `ui/src/lib/simulator.js` → `invoke()` precisa responder o mesmo comando (modo browser).
   - `ui/src/app.js` → chamada `api.*` e atualização de DOM.
4. Rodar `cd src-tauri && cargo test --lib` (o teste `snapshot_has_five_tracks_and_fx_catalog` protege o contrato).
5. `graphify update .`.

## Regras
- Nada de `Mutex` no audio thread: parâmetros vão para atomics em `state.rs`, comandos pelo canal `crossbeam`.
- Emitter de UI é uma thread única com `sleep(16ms)`; não criar outra.
- Comando que muda estado de track deve enviar `TrackCommand` pelo canal, não mutar `Track` diretamente.

## Arquivos
- `src-tauri/src/lib.rs` — comandos + snapshot + `run()`.
- `src-tauri/src/state.rs` — atomics e enums compartilhados.
- `ui/src/lib/bridge.js` — detecção `window.__TAURI__`.
- `ui/src/lib/simulator.js` — máquina de estados in-browser.
