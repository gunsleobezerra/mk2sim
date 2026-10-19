# AGENTS.md

## Graphify (knowledge graph do repositório)

Este projeto usa Graphify (`graphifyy`) para manter um knowledge graph do código.

- Comece por `graphify query "<pergunta>"` se `graphify-out/graph.json` existir.
- Use `graphify path` / `graphify affected` para mapear dependências antes de alterar componentes compartilhados.
- Após criar/mover/remover módulos, rode `graphify update .` para atualizar o grafo.

Detalhes: `.github/instructions/graphify.instructions.md`.

## Estrutura

- `src-tauri/` — backend Rust (Tauri 2): `audio/` (engine, clock, track, plugin host), `dsp/` (effects, rhythm), `midi/` (controller), `state.rs`.
- `ui/` — frontend JS puro: `src/components/`, `src/lib/` (`bridge.js`, `simulator.js`, `dom.js`), `styles/rc505_hardware.css`.
- Ponte Rust↔JS: comandos/eventos Tauri devem ser espelhados em `ui/src/lib/bridge.js` e `ui/src/lib/simulator.js`.

## Comandos

```bash
cargo build            # dentro de src-tauri/
npm run tauri dev      # se configurado na raiz
graphify update .      # atualizar knowledge graph
```
