# skm-python

Python bindings for SKM Agent Skill Engine.

## Installation

### From PyPI (when published)

```bash
pip install skm
```

### From source (development)

```bash
# Install maturin
pip install maturin

# Build and install locally
cd crates/skm-python
maturin develop
```

## Usage

```python
from skm import SkillRegistry, CascadeSelector, TriggerStrategy

# Create a registry from skill directories
registry = SkillRegistry(["/path/to/skills"])

# List all skills
print(registry.list())

# Get skill metadata
meta = registry.get("my-skill")
print(f"Skill: {meta.name}")
print(f"Description: {meta.description}")
print(f"Triggers: {meta.triggers}")

# Fast trigger-based selection
trigger = TriggerStrategy.from_registry(registry)
results = trigger.select("extract text from pdf", registry)
for r in results:
    print(f"{r.skill}: {r.score:.2f} ({r.confidence})")

# Cascading selection (tries fast methods first)
selector = CascadeSelector(registry)
results = selector.select("summarize this document", registry)

# With full stats
outcome = selector.select_with_stats("query", registry)
print(f"Latency: {outcome['total_latency_ms']}ms")
print(f"Strategies used: {outcome['strategies_used']}")
```

### Embedding Providers

```python
from skm import BgeM3Provider, MiniLmProvider

# BGE-M3: 1024-dim, multilingual, supports Chinese
bge = BgeM3Provider(cache_size=1000)
embedding = bge.embed("Hello, world!")
print(f"Dimensions: {len(embedding)}")  # 1024

# MiniLM: 384-dim, English only, faster
minilm = MiniLmProvider()
embeddings = minilm.embed_batch(["text1", "text2", "text3"])
```

### Enforcement Pipeline

```python
from skm import EnforcementPipeline

# Create pipeline (allow-all by default)
pipeline = EnforcementPipeline()

# Pre-activation check
decision = pipeline.check_before(
    "my-skill", 
    "user query",
    user_id="user123",
    session_id="session456"
)

if decision.is_allowed():
    # Execute skill...
    output = "skill output"
    
    # Post-execution check
    after = pipeline.check_after("my-skill", output)
    if after.modified_output:
        output = after.modified_output
```

## Building

```bash
# Build wheel
maturin build --release

# Build with specific features
maturin build --release --features embed-minilm
```

## Features

- `embed-bge-m3` (default): BGE-M3 embedding provider
- `embed-minilm`: MiniLM embedding provider (smaller, faster)
