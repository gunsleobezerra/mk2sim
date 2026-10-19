---
description: 'Graphify: como consultar e atualizar o knowledge graph do repositório'
applyTo: '**'
---

# Graphify — Instruções para o Agente

Graphify (`graphifyy`) analisa o repositório e gera um knowledge graph persistente. Use os artefatos para navegar dependências em vez de varrer a árvore de arquivos.

Instruções base geradas pelo instalador: `.github/copilot-instructions.md`.

## Artefatos

Saída em `graphify-out/` na raiz do projeto:

| Arquivo | Conteúdo |
| --- | --- |
| `graphify-out/graph.json` | Nós, arestas e comunidades |
| `graphify-out/GRAPH_REPORT.md` | Visão arquitetural e comunidades |
| `graphify-out/graph.html` | Visualização interativa |
| `graphify-out/wiki/index.md` | Wiki navegável (quando gerada) |

## Comandos

```bash
# Consultas (primeira ação para perguntas de arquitetura/estrutura)
graphify query "<pergunta>"
graphify path "<A>" "<B>"
graphify explain "<conceito>"
graphify affected "<símbolo>" --depth 2
graphify god-nodes --top 10

# Construir / atualizar o grafo
graphify update .              # re-extrai código (AST, sem LLM) — use no dia a dia
graphify extract . --code-only # extração headless só de código (sem API key)
graphify extract .             # extração completa (AST + semântica LLM; exige API key)

# Extras
graphify hook install          # hooks git pós-commit/pós-checkout
graphify export wiki           # gera graphify-out/wiki/
```

Não existe `graphify .` como comando direto — dentro do Copilot Chat o atalho é `/graphify`.

## Fluxo obrigatório

1. **Consulta primeiro** — se `graphify-out/graph.json` existe, comece por `graphify query "<pergunta>"`. Só leia código-fonte quando for modificar/depurar, quando o grafo não tiver detalhe, ou quando estiver desatualizado.
2. **Dependências antes de refatorar** — use `graphify affected` / `graphify path` para mapear consumidores antes de alterar componentes compartilhados.
3. **Refresh após mudanças estruturais** — depois de criar, mover ou remover múltiplos arquivos/módulos, rode `graphify update .`.
4. **Relatório só para visão ampla** — `GRAPH_REPORT.md` é para revisão arquitetural, não para consultas pontuais.

## Regras específicas deste repositório

- Arquitetura: app Tauri (`src-tauri/`) + UI web (`ui/`). Camadas Rust: `audio/`, `dsp/`, `midi/`, `state.rs`.
- Ao mexer em `audio/engine.rs`, `audio/track.rs` ou `dsp/effects.rs`, consulte o grafo primeiro para localizar consumidores no frontend (`ui/src/lib/bridge.js`, `ui/src/components/*`).
- Ponte Rust↔JS: mudanças em comandos/eventos Tauri exigem checagem cruzada em `ui/src/lib/bridge.js` e `ui/src/lib/simulator.js`.
- `graphify-out/graph.json` e `GRAPH_REPORT.md` podem ser versionados para contexto do agente; `graph.html` é artefato local.
