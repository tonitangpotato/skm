# Mini Agent Example

A minimal working example demonstrating SKM trigger-based skill selection.

## What This Shows

This example loads 12 realistic skills from the `skills/` directory and provides an interactive REPL where you can test skill matching against user queries.

**Key concepts demonstrated:**
- Loading skills from a directory with `SkillRegistry`
- Building a `TriggerStrategy` from the registry
- Matching queries against skills
- Displaying match results with scores and confidence levels

## Running the Example

From the repository root:

```bash
cd examples/mini-agent
cargo run
```

Or with verbose logging:

```bash
RUST_LOG=debug cargo run
```

## Example Session

```
╔══════════════════════════════════════════════════════════════╗
║                    SKM Mini Agent Demo                       ║
╚══════════════════════════════════════════════════════════════╝

Loading skills from "./skills"...
Loaded 12 skills.

query> deploy to production
  ✅ Matched 1 skill(s):
  ─────────────────────────────────────────────────────────────
  🎯 deployment (score: 0.82, confidence: High)
    └─ Deploy applications, manage releases, and handle production deployments
    └─ reason: Matched keyword

query> translate this to Spanish
  ✅ Matched 1 skill(s):
  ─────────────────────────────────────────────────────────────
  🎯 translate (score: 0.80, confidence: High)
    └─ Translate text between languages with cultural context
    └─ reason: Matched keyword

query> what's the weather?
  ✅ Matched 1 skill(s):
  ─────────────────────────────────────────────────────────────
  ✓ weather (score: 0.75, confidence: High)
    └─ Get current weather conditions and forecasts for any location
    └─ reason: Matched keyword

query> list
Available skills:
─────────────────────────────────────────────────────────────
  • api-testing - Test HTTP APIs, make curl requests, and debug endpoints
  • code-review - Review code changes, provide feedback on PRs, and suggest improvements
  • database-query - Write and execute SQL queries, analyze database schemas
  • deployment - Deploy applications, manage releases
  • email-compose - Draft and compose professional emails
  • file-search - Find files and search within files
  • git-operations - Manage git repositories, branches, commits
  • image-gen - Generate images from text descriptions
  • summarize - Summarize text, documents, articles
  • translate - Translate text between languages
  • weather - Get current weather conditions and forecasts
  • web-scraping - Fetch web pages, extract content from URLs
```

## Skills Included

| Skill | Triggers |
|-------|----------|
| `web-scraping` | scrape, fetch page, URLs (http://, https://) |
| `code-review` | review, PR, pull request, check this code |
| `database-query` | SQL, query, database, SELECT, INSERT |
| `deployment` | deploy, ship, release, push to production |
| `git-operations` | git, commit, branch, merge, push |
| `weather` | weather, forecast, temperature |
| `email-compose` | email, send mail, compose |
| `file-search` | find file, search, locate, grep |
| `api-testing` | test API, curl, endpoint, HTTP |
| `summarize` | summarize, TLDR, key points |
| `translate` | translate, 翻译, to Spanish, to Japanese |
| `image-gen` | generate image, draw, create picture |

## SKILL.md Format

Each skill uses the [agentskills.io](https://agentskills.io) format:

```yaml
---
name: skill-name
description: Brief description for selection
metadata:
  triggers: "keyword1, keyword2, regex-pattern"
  tags: "category1, category2"
  allowed_tools: "tool1, tool2"
---

# Skill Instructions

The actual instructions for the agent...
```

## Extending This Example

### Add Semantic Selection

To add embedding-based semantic selection (for fuzzy matching when triggers miss):

```rust
use skm_embed::BgeM3Provider;
use skm_select::{CascadeSelector, SemanticConfig};

// Build cascade: triggers first, then semantic
let embedder = BgeM3Provider::new()?;
let selector = CascadeSelector::builder()
    .with_triggers()
    .with_semantic(Arc::new(embedder), SemanticConfig::default())
    .build(&registry)?;
```

### Add LLM Fallback

For ultimate accuracy when other methods are uncertain:

```rust
use skm_select::{LlmStrategy, LlmClient};

// Implement your LLM client
struct MyLlmClient { /* ... */ }

impl LlmClient for MyLlmClient {
    async fn classify(&self, query: &str, skills: &[&SkillMetadata]) -> Result<SelectionResult, LlmError> {
        // Call your LLM
    }
}

let selector = CascadeSelector::builder()
    .with_triggers()
    .with_semantic(Arc::new(embedder), SemanticConfig::default())
    .with_llm(Arc::new(MyLlmClient::new()))
    .build(&registry)?;
```

## Next Steps

- Explore `skm-disclose` for progressive loading (summaries first, full content on demand)
- Check `skm-enforce` for adding safety policies and hooks
- Try `skm-learn` for usage analytics and trigger optimization
