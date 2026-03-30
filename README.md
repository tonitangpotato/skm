<p align="center">
  <h1 align="center">skm</h1>
  <p align="center">
    <strong>The missing runtime for Agent Skills</strong>
  </p>
  <p align="center">
    Selection, enforcement, and learning for SKILL.md — so you don't have to build it yourself.
  </p>
</p>

<p align="center">
  <a href="https://crates.io/crates/skm"><img src="https://img.shields.io/crates/v/skm.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/skm"><img src="https://docs.rs/skm/badge.svg" alt="Documentation"></a>
  <a href="https://github.com/tonitangpotato/skm/actions"><img src="https://github.com/tonitangpotato/skm/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
</p>

---

## What is skm?

**skm** is a Rust library that turns [Agent Skills](https://agentskills.io) from static markdown files into a smart runtime. It handles everything between "user asks a question" and "agent knows which skill to use":

```
User Query → Trigger Match (µs) → Semantic Search (ms) → LLM Fallback (s) → Selected Skill
```

Think of it as the brain that picks the right tool for the job — except it's 1000x faster than asking an LLM every time.

## 30-Second Quickstart

```toml
# Cargo.toml
[dependencies]
skm = "0.1"
tokio = { version = "1", features = ["full"] }
```

```rust
use skm::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load skills from a directory
    let registry = SkillRegistry::new(&["./skills"]).await?;
    
    // Build a trigger-based selector (microsecond latency)
    let selector = TriggerStrategy::from_registry(&registry).await?;
    
    // Match a query
    let catalog = registry.catalog().await;
    let refs: Vec<_> = catalog.iter().collect();
    let ctx = SelectionContext::new();
    
    let results = selector.select("deploy to production", &refs, &ctx).await?;
    
    for result in results {
        println!("{}: {:.2} confidence", result.skill, result.score);
    }
    
    Ok(())
}
```

That's it. Your agent now has skill selection.

## Why skm?

Every AI agent framework re-implements the same thing:

1. **Parse** skill files → custom parsers everywhere
2. **Select** which skill to use → "just ask the LLM every time" (slow, expensive)
3. **Load** skill instructions → copy-paste the whole file into context

This is wasteful. SKM gives you:

| Problem | Without SKM | With SKM |
|---------|-------------|----------|
| Skill selection | LLM call every time (~1-2s) | Trigger match in 50µs, semantic in 5ms |
| 100 skills in context | Dump all 100 into prompt | Progressive disclosure: summaries first |
| Skill misbehavior | Hope the LLM follows instructions | Policy enforcement + hooks |
| Trigger accuracy | Manual testing, guesswork | Automated test harness + optimizer |

## How It Works

SKM uses a **cascade** architecture: try the fastest method first, escalate only if needed.

```
┌─────────────────────────────────────────────────────────────────────┐
│                         User Query                                   │
└─────────────────────────┬───────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  STAGE 1: Trigger Matching                                    ~50µs │
│  ├─ Regex patterns (^deploy.*)                                      │
│  ├─ Keyword matching (deploy, ship, release)                        │
│  └─ Implicit name matching (deployment-skill)                       │
│                                                                      │
│  → High confidence match? DONE. Skip the rest.                      │
└─────────────────────────┬───────────────────────────────────────────┘
                          │ no definite match
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  STAGE 2: Semantic Similarity                                  ~5ms │
│  ├─ BGE-M3 embeddings (100+ languages)                              │
│  ├─ Cosine similarity against skill descriptions                    │
│  └─ Local inference via ONNX (no API calls)                         │
│                                                                      │
│  → Above threshold? Return ranked results.                          │
└─────────────────────────┬───────────────────────────────────────────┘
                          │ still uncertain
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  STAGE 3: LLM Classification                                  ~1-2s │
│  ├─ Few-shot examples from skill metadata                           │
│  ├─ Structured output for skill selection                           │
│  └─ Your LLM, your API key                                          │
│                                                                      │
│  → Final answer with reasoning.                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Result**: 95%+ of queries resolve in Stage 1 or 2. LLM calls are the exception, not the rule.

## Features

### 🎯 Cascade Selection
Three-tier matching: regex triggers (µs) → semantic embeddings (ms) → LLM classification (s). Each stage can short-circuit on confident matches.

### 📦 Progressive Disclosure  
Don't dump 50 skill files into your context window. SKM gives you:
- **Level 0**: Names + descriptions (for selection)
- **Level 1**: Summaries + examples (for refinement)  
- **Level 2**: Full instructions (only what you need)

Token-budget aware: "Give me the best skills that fit in 4000 tokens."

### 🔒 Enforcement Pipeline
Safety isn't a prompt hack. SKM provides:
- **Pre-activation hooks**: Validate before the skill runs
- **Policy engine**: Declarative rules (require tags, block patterns)
- **Post-execution validators**: Check outputs before returning

### 📊 Learning Loop
Improve over time:
- **Usage analytics**: Which skills get selected? Which get rejected?
- **Trigger test harness**: Automated testing of your trigger patterns
- **Description optimizer**: LLM-assisted rewriting for better semantic matching

### 🌍 Multilingual
BGE-M3 embeddings support 100+ languages. Your Chinese skill descriptions work just as well as English ones.

### ⚡ Offline-First
Zero required network calls. Embeddings run locally via ONNX. LLM fallback is opt-in.

## Comparison

| Feature | skm | agent-skills | mcp-tool-filter | DIY |
|---------|-----|--------------|-----------------|-----|
| Language | Rust + bindings | TypeScript | TypeScript | varies |
| SKILL.md parsing | ✅ | ✅ | ❌ | manual |
| Trigger matching | ✅ (µs) | ❌ | ❌ | manual |
| Semantic selection | ✅ (local) | ❌ | ✅ (API) | manual |
| Progressive disclosure | ✅ | ❌ | ❌ | manual |
| Enforcement/hooks | ✅ | ❌ | ❌ | manual |
| Learning/analytics | ✅ | ❌ | ❌ | manual |
| Filesystem watching | ✅ | ❌ | ❌ | manual |

## Performance

Benchmarks on M1 MacBook Pro, 50 skills:

| Operation | Latency | Notes |
|-----------|---------|-------|
| Trigger match (hit) | **47µs** | Single regex/keyword match |
| Trigger match (miss) | **120µs** | Full scan, no match |
| Semantic search (BGE-M3) | **4.2ms** | Including embedding query |
| Semantic search (MiniLM) | **1.8ms** | Smaller model, English-only |
| Registry load | **15ms** | 50 skills, metadata only |
| Full skill load | **0.3ms** | Single skill, lazy load |

## Crate Structure

| Crate | Description |
|-------|-------------|
| `skm-core` | SKILL.md parser, registry, filesystem watcher |
| `skm-embed` | Embedding providers (BGE-M3, MiniLM, OpenAI API) |
| `skm-select` | Selection strategies + cascade orchestration |
| `skm-disclose` | Progressive disclosure, token budgeting |
| `skm-enforce` | Hooks, policy engine, validators |
| `skm-learn` | Usage metrics, test harness, optimizer |
| `skm-cli` | Developer CLI (`skm init`, `select`, `bench`) |
| `skm` | Facade crate with `prelude` re-exports |
| `skm-python` | Python bindings (PyO3) |
| `skm-node` | Node.js bindings (NAPI-RS) |

## Installation

### Rust

```toml
[dependencies]
skm = "0.1"

# Or pick specific crates:
skm-core = "0.1"    # Just parsing
skm-select = "0.1"  # Selection without embeddings
```

Feature flags for embedding providers:

```toml
# Default: BGE-M3 (1024d, multilingual)
skm = { version = "0.1", features = ["embed-bge-m3"] }

# Faster, English-only
skm = { version = "0.1", features = ["embed-minilm"] }

# OpenAI API (requires API key)
skm = { version = "0.1", features = ["embed-openai"] }

# No embeddings (trigger-only, smallest binary)
skm = { version = "0.1", features = ["no-embed"] }
```

### Python

```bash
pip install skm
```

```python
from skm import SkillRegistry, TriggerStrategy

registry = SkillRegistry(["./skills"])
selector = TriggerStrategy.from_registry(registry)

results = selector.select("deploy to production")
for r in results:
    print(f"{r.skill}: {r.score:.2f}")
```

### Node.js

```bash
npm install @skm/core
```

```javascript
import { SkillRegistry, TriggerStrategy } from '@skm/core';

const registry = await SkillRegistry.create(['./skills']);
const selector = await TriggerStrategy.fromRegistry(registry);

const results = await selector.select('deploy to production');
results.forEach(r => console.log(`${r.skill}: ${r.score.toFixed(2)}`));
```

### HTTP API

For other languages, run the SKM server:

```bash
skm serve --port 8080 --skills ./skills
```

```bash
curl -X POST http://localhost:8080/select \
  -H "Content-Type: application/json" \
  -d '{"query": "deploy to production"}'
```

## CLI

```bash
# Install
cargo install skm-cli

# Scaffold a new skill
skm init my-skill

# Validate all skills in a directory
skm validate ./skills

# Test selection interactively
skm select "user query" --skills ./skills

# Benchmark selection latency
skm bench ./skills --queries 1000

# Run trigger test harness
skm test ./skills

# Show usage analytics
skm stats ./skills

# Optimize skill descriptions
skm optimize ./skills --model gpt-4o-mini
```

## Examples

See [`examples/mini-agent/`](./examples/mini-agent/) for a complete working example with 12 realistic skills.

## Documentation

- [API Documentation](https://docs.rs/skm)
- [Design Document](./DESIGN.md) — Architecture deep-dive
- [Testing Guide](./docs/TESTING.md) — How to test SKM features

## Status

SKM is in active development. The API is stabilizing but may change before 1.0.

- [x] v0.1 — Foundation (core parsing, trigger selection)
- [x] v0.2 — Embeddings (semantic selection, BGE-M3/MiniLM)
- [x] v0.3 — Safety (hooks, policy engine, validators)
- [x] v0.4 — Learning (metrics, test harness, analytics)
- [x] v0.5 — Optimization (description optimizer, few-shot)
- [ ] v1.0 — Production (stable API, full docs, crates.io)

## Contributing

Contributions welcome! Please read the [contributing guidelines](./CONTRIBUTING.md) first.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
