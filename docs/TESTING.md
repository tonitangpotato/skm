# Testing Guide

This guide covers how to run tests, what each crate tests, and how to test advanced features that require additional setup.

## Quick Start

```bash
# Run all fast tests (no model downloads, no network)
cargo test

# Run all tests including ignored (requires model files)
cargo test -- --include-ignored

# Run doc tests
cargo test --doc

# Run tests for a specific crate
cargo test -p skm-core
cargo test -p skm-select
```

## Test Commands Reference

| Command | What it tests | Speed |
|---------|--------------|-------|
| `cargo test` | Unit tests, integration tests | ~5s |
| `cargo test --doc` | Documentation examples | ~10s |
| `cargo test -- --include-ignored` | All tests including embedding models | ~30s |
| `cargo test -p skm-core` | Core parsing/registry only | ~2s |
| `cargo test -p skm-select` | Selection strategies | ~3s |
| `cargo test -p skm-embed -- --include-ignored` | Embedding providers | ~20s |

## Test Matrix

### skm-core

Tests for parsing and registry functionality:

| Test Module | Coverage |
|-------------|----------|
| `parser::tests` | YAML frontmatter parsing, markdown body extraction, validation |
| `schema::tests` | SkillName validation, triggers/tags extraction, token estimation |
| `registry::tests` | Directory scanning, lazy loading, refresh, stats tracking |
| `watcher::tests` | Filesystem event handling |

```bash
cargo test -p skm-core
```

### skm-select

Tests for selection strategies:

| Test Module | Coverage |
|-------------|----------|
| `trigger::tests` | Keyword matching, regex patterns, case insensitivity, scoring |
| `semantic::tests` | Embedding search, threshold tuning, candidate filtering |
| `cascade::tests` | Multi-stage selection, early exit, result merging |

```bash
cargo test -p skm-select
```

### skm-embed

Tests for embedding providers (some require model downloads):

| Test Module | Coverage | Requires |
|-------------|----------|----------|
| `bge_m3::tests` | BGE-M3 embedding generation | Model download (~1.5GB) |
| `minilm::tests` | MiniLM embedding generation | Model download (~30MB) |
| `index::tests` | Embedding index, similarity search | Fast |

```bash
# Fast tests only
cargo test -p skm-embed

# Include embedding model tests
cargo test -p skm-embed -- --include-ignored
```

### skm-disclose

Tests for progressive disclosure:

| Test Module | Coverage |
|-------------|----------|
| `level::tests` | Disclosure level generation (L0/L1/L2) |
| `budget::tests` | Token budget fitting, skill selection under constraints |
| `context::tests` | Context manager state |

```bash
cargo test -p skm-disclose
```

### skm-enforce

Tests for enforcement pipeline:

| Test Module | Coverage |
|-------------|----------|
| `hook::tests` | Before/after hook execution, abort handling |
| `policy::tests` | Policy rule parsing, evaluation, combining |
| `validator::tests` | Output validation, regex checks |
| `pipeline::tests` | Full enforcement flow |

```bash
cargo test -p skm-enforce
```

### skm-learn

Tests for learning and optimization:

| Test Module | Coverage |
|-------------|----------|
| `metrics::tests` | Metric collection, aggregation, persistence |
| `harness::tests` | Trigger test execution, reporting |
| `optimizer::tests` | Description optimization suggestions |
| `analytics::tests` | Usage pattern analysis |

```bash
cargo test -p skm-learn
```

## Testing Advanced Features

### Semantic Selection (skm-embed)

Semantic selection requires embedding models. On first run, models are downloaded to a cache directory.

**Setup:**

```bash
# Run with model download (first time only, ~1.5GB for BGE-M3)
RUST_LOG=debug cargo test -p skm-embed -- --include-ignored
```

**Test queries:**

```rust
// The tests verify that semantically similar queries match
// even when no trigger keywords are present

// Example: "ship my app" should match deployment skill
// even though "ship" isn't an explicit trigger
```

**Manual testing:**

```bash
# Use the CLI to test semantic matching
skm select "ship my application to users" --skills ./examples/mini-agent/skills --strategy semantic
```

### Enforcement (skm-enforce)

**Testing policies:**

Create a test policy file:

```yaml
# test-policy.yaml
rules:
  - name: require-allowed-tools
    condition:
      all:
        - field: metadata.allowed_tools
          operator: is_set
    action: allow
    
  - name: block-dangerous-patterns
    condition:
      any:
        - field: instructions
          operator: contains
          value: "rm -rf"
        - field: instructions
          operator: matches
          value: "sudo.*"
    action: deny
    message: "Dangerous pattern detected"
```

**Test hooks:**

```rust
use skm_enforce::{BeforeSkillActivation, HookDecision, EnforcementContext};

struct TestHook;

impl BeforeSkillActivation for TestHook {
    async fn before_activation(
        &self,
        skill: &Skill,
        ctx: &EnforcementContext,
    ) -> HookDecision {
        if skill.name.as_str() == "dangerous-skill" {
            HookDecision::Abort("Blocked for testing".into())
        } else {
            HookDecision::Continue
        }
    }
}
```

### Learning (skm-learn)

**Metrics collection testing:**

```bash
# Run the CLI with metrics enabled
SKM_METRICS_DIR=/tmp/skm-metrics skm select "deploy" --skills ./skills

# Check collected metrics
cat /tmp/skm-metrics/selections.jsonl
```

**Trigger test harness:**

Create test cases:

```yaml
# tests/trigger-tests.yaml
test_cases:
  - query: "deploy to production"
    expected: deployment
    
  - query: "what's the weather like"
    expected: weather
    
  - query: "translate this to Spanish"
    expected: translate
    
  - query: "random gibberish query"
    expected: null  # Should not match
```

Run the harness:

```bash
skm test ./skills --cases tests/trigger-tests.yaml
```

**Optimizer testing:**

```bash
# Generate optimization suggestions (requires LLM API)
skm optimize ./skills --model gpt-4o-mini --dry-run
```

### Progressive Disclosure (skm-disclose)

**Token budget scenarios:**

```rust
use skm_disclose::{ContextManager, TokenBudget, DisclosureLevel};

// Test fitting skills within a budget
let manager = ContextManager::new(registry);

// Scenario 1: Tight budget (4K tokens)
let budget = TokenBudget::new(4000);
let payload = manager.prepare_context(&["deployment", "git-ops"], budget).await?;
assert!(payload.total_tokens <= 4000);

// Scenario 2: Large budget (100K tokens)
let budget = TokenBudget::new(100_000);
let payload = manager.prepare_context(&skills, budget).await?;
// Should include full L2 content
```

**Manual testing:**

```bash
# Show disclosure levels for skills
skm list ./skills --disclosure

# Get specific level
skm show deployment --level 0  # Name + description
skm show deployment --level 1  # + examples
skm show deployment --level 2  # Full content
```

### HTTP Server (skm serve)

**Starting the server:**

```bash
skm serve --port 8080 --skills ./examples/mini-agent/skills
```

**Endpoint testing with curl:**

```bash
# Health check
curl http://localhost:8080/health

# List skills
curl http://localhost:8080/skills

# Select skill
curl -X POST http://localhost:8080/select \
  -H "Content-Type: application/json" \
  -d '{"query": "deploy to production"}'

# Select with options
curl -X POST http://localhost:8080/select \
  -H "Content-Type: application/json" \
  -d '{
    "query": "deploy to production",
    "strategy": "cascade",
    "limit": 3,
    "min_score": 0.5
  }'

# Get skill details
curl http://localhost:8080/skills/deployment

# Get specific disclosure level
curl "http://localhost:8080/skills/deployment?level=1"
```

**Response format:**

```json
{
  "results": [
    {
      "skill": "deployment",
      "score": 0.85,
      "confidence": "High",
      "strategy": "trigger",
      "reasoning": "Matched keyword 'deploy'"
    }
  ],
  "latency_ms": 0.12,
  "strategy_used": "trigger"
}
```

### Full Cascade Testing

Test the complete cascade flow: trigger → semantic → LLM:

```bash
# Setup: ensure embedding model is downloaded
cargo test -p skm-embed -- --include-ignored

# Test cascade behavior
RUST_LOG=debug skm select "ship my new feature to users" \
  --skills ./examples/mini-agent/skills \
  --strategy cascade \
  --verbose
```

Expected behavior:

1. **Trigger match**: Checks for keywords like "ship", "deploy"
2. **Semantic fallback**: If trigger score < 0.8, runs embedding search
3. **LLM fallback**: If semantic score < 0.7 and LLM is configured

To test with LLM fallback:

```bash
# Configure your LLM API
export OPENAI_API_KEY=sk-...

# Run with LLM enabled
skm select "help me with my application release process" \
  --skills ./skills \
  --strategy cascade \
  --llm-model gpt-4o-mini
```

## Writing New Tests

### Conventions

- Test files live alongside source in `src/` as `#[cfg(test)] mod tests`
- Integration tests go in `tests/` directory
- Test fixtures go in `tests/fixtures/`

### Test Fixtures

Create SKILL.md fixtures for testing:

```
tests/
  fixtures/
    skills/
      test-skill/
        SKILL.md
      another-skill/
        SKILL.md
```

```rust
// In your test
let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests/fixtures/skills");
let registry = SkillRegistry::new(&[&fixture_dir]).await?;
```

### Assertion Helpers

```rust
// Assert skill was selected with minimum score
fn assert_selected(results: &[SelectionResult], skill: &str, min_score: f32) {
    let found = results.iter().find(|r| r.skill.as_str() == skill);
    assert!(found.is_some(), "Expected skill {} to be selected", skill);
    assert!(
        found.unwrap().score >= min_score,
        "Expected score >= {}, got {}",
        min_score,
        found.unwrap().score
    );
}

// Assert skill was NOT selected
fn assert_not_selected(results: &[SelectionResult], skill: &str) {
    let found = results.iter().find(|r| r.skill.as_str() == skill);
    assert!(found.is_none(), "Expected skill {} to NOT be selected", skill);
}
```

## CI Considerations

### Fast CI (default)

```yaml
# .github/workflows/ci.yml
- name: Run tests
  run: cargo test
```

This runs all non-ignored tests (~5 seconds).

### Full CI (with models)

```yaml
# .github/workflows/full-test.yml
- name: Cache embedding models
  uses: actions/cache@v4
  with:
    path: ~/.cache/huggingface
    key: embedding-models-${{ runner.os }}
    
- name: Run all tests
  run: cargo test -- --include-ignored
```

### Test Categories

| Category | CI Strategy | When to Run |
|----------|-------------|-------------|
| Unit tests | Always | Every PR |
| Integration tests | Always | Every PR |
| Embedding tests | Cached | Weekly, release |
| LLM tests | Skip in CI | Manual only |
| Benchmark tests | Scheduled | Nightly |

### Environment Variables

```bash
# Skip slow tests in CI
SKM_SKIP_SLOW_TESTS=1 cargo test

# Use specific model cache
FASTEMBED_CACHE_PATH=/path/to/models cargo test

# Enable verbose output
RUST_LOG=debug cargo test
```

## Benchmarks

```bash
# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench selection

# Generate HTML report
cargo bench -- --save-baseline main
```

Benchmark scenarios:

- `trigger_match_hit`: Time to match a query that hits
- `trigger_match_miss`: Time to scan all triggers with no match
- `semantic_search_50`: Semantic search over 50 skills
- `semantic_search_500`: Semantic search over 500 skills
- `registry_load`: Time to load skill directory
- `skill_parse`: Time to parse single SKILL.md
