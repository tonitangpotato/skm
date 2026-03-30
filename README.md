# skm — Agent Skill Engine

The missing runtime layer for [Agent Skills](https://agentskills.io): selection, enforcement, and optimization as a framework-agnostic Rust crate.

## Why

The Agent Skills standard (`SKILL.md`) defines a portable format for AI agent instructions, adopted by Claude, Codex, Copilot, Cursor, Gemini CLI, and 20+ other platforms. But the standard only defines the **format** — not the **runtime**.

Every platform re-implements skill discovery, selection, and loading from scratch. `skm` is the reusable runtime that handles the full lifecycle:

```
Parse → Select → Disclose → Enforce → Learn
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  skm (facade)                   │
├──────────┬──────────┬───────────┬───────────────┤
│ skm-core │ skm-embed│ skm-select│  skm-disclose │
│ parse    │ BGE-M3   │ trigger µs│  progressive  │
│ registry │ MiniLM   │ semantic  │  token budget │
│ watch    │ SIMD     │ LLM      s│               │
├──────────┴──────────┼───────────┼───────────────┤
│    skm-enforce      │ skm-learn │   skm-cli     │
│    hooks & policy   │ metrics   │   `skm` cmd   │
│    validators       │ harness   │               │
│    pipeline         │ optimizer │               │
└─────────────────────┴───────────┴───────────────┘
```

## Key Features

- **Standards-native** — `SKILL.md` (agentskills.io) is the only skill format. No proprietary extensions.
- **Cascade selection** — Regex triggers (µs) → semantic similarity (ms) → LLM classification (s). Fastest match wins.
- **Progressive disclosure** — Load skill summaries first, full content on demand. Token-budget aware.
- **Enforcement** — Pre/post hooks, policy engine, output validators. Safety as code, not prompt.
- **Closed-loop learning** — Usage metrics, trigger test harness, description optimizer.
- **Framework-agnostic** — Provides traits (`EmbeddingProvider`, `LlmClient`, `SelectionStrategy`) that any agent framework implements.
- **Multilingual** — BGE-M3 embeddings support 100+ languages out of the box.
- **Offline-first** — Zero required network. ONNX inference via `fastembed`.

## Quick Start

```toml
[dependencies]
skm = "0.1"
```

```rust
use skm::prelude::*;

// 1. Build registry from skill directories
let registry = SkillRegistry::new(&["./skills"]).await?;

// 2. Create cascade selector (trigger → semantic → LLM)
let selector = CascadeSelector::builder()
    .with_trigger()
    .with_semantic(embedder)
    .build();

// 3. Select skills for a user query
let results = selector.select("deploy to production", &registry).await?;
```

## Crates

| Crate | Description |
|-------|-------------|
| `skm-core` | SKILL.md parser, registry, filesystem watcher |
| `skm-embed` | Embedding providers (BGE-M3, MiniLM, API), SIMD ops, index |
| `skm-select` | Selection strategies (trigger, semantic, LLM, few-shot) + cascade |
| `skm-disclose` | Progressive disclosure levels, token estimator, context manager |
| `skm-enforce` | Hook system, policy engine, output validators, enforcement pipeline |
| `skm-learn` | Usage metrics, trigger test harness, analytics, description optimizer |
| `skm-cli` | Developer CLI (`skm init`, `validate`, `select`, `bench`, `optimize`) |
| `skm` | Facade crate with `prelude` re-exports |

## Feature Flags

```toml
# Embedding providers (default: bge-m3)
skm-embed = { features = ["embed-bge-m3"] }     # BGE-M3 (1024d, 100+ langs)
skm-embed = { features = ["embed-minilm"] }      # MiniLM (384d, English, fast)
skm-embed = { features = ["embed-openai"] }       # OpenAI API
skm-embed = { features = ["no-embed"] }           # No embedding (trigger-only)

# Analytics
skm-learn = { features = ["analytics-memory"] }   # In-memory analytics store
```

## CLI

```bash
skm init my-skill              # Scaffold a new SKILL.md
skm validate ./skills/         # Validate all skills
skm list ./skills/             # List registered skills
skm select "user query"        # Test cascade selection
skm bench ./skills/            # Benchmark selection latency
skm test ./skills/             # Run trigger test harness
skm stats ./skills/            # Show usage analytics
skm optimize ./skills/         # Optimize skill descriptions
```

## Design

See [DESIGN.md](./DESIGN.md) for the complete design document covering architecture, data models, selection cascade, enforcement pipeline, and learning loop.

## Status

- [x] v0.1 — Foundation (core, trigger selection, CLI)
- [x] v0.2 — Intelligence (embeddings, semantic selection, disclosure)
- [x] v0.3 — Safety (hooks, policy engine, validators)
- [x] v0.4 — Learning (metrics, test harness, analytics)
- [x] v0.5 — Optimization (description optimizer, LLM selection, few-shot)
- [ ] v1.0 — Production (HTTP API, Prometheus, docs, crates.io, PyO3)

## License

MIT OR Apache-2.0
