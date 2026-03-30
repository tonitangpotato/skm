# agent-skill-engine (skm) — Complete Design Document

agent-skill-engine — Complete Design Document
The missing runtime layer for Agent Skills: selection, enforcement, and optimization as a
framework-agnostic Rust crate.
1. Problem Statement
The Agent Skills open standard (agentskills.io) defines a portable format for packaging
instructions that AI agents can discover and execute. Adopted by Claude, Codex, Copilot,
Cursor, Gemini CLI, and 20+ other platforms, SKILL.md is now the de facto standard.
But the standard only defines the format — not the runtime.
Every agent platform re-implements skill discovery, selection, and loading from scratch.
None of them expose a reusable, framework-agnostic runtime that handles the full skill
lifecycle:
Lifecycle
Stage
Current State of the Art
Gap
Parse /
Validate
agent-skills  crate (Govcraft), skillc
CLI
 Covered
Distribute
skillshub , npx skills
 Covered
Select
Each framework rolls its own; Portkey mcp-
tool-filter  (TypeScript only)
 No reusable Rust
library
Load
(Progressive)
Described in spec, implemented ad-hoc per
platform
 No standalone
implementation
Enforce
Strands hooks, LangGraph node guards — all
framework-coupled
 No framework-
agnostic enforcement
Learn /
Optimize
skillc stats  (basic usage counting)
 No trigger testing, no
auto-optimization
agent-skill-engine  fills every gap in a single workspace.
2. Design Principles
1. Standards-native — SKILL.md (agentskills.io) is the only skill format. No forks, no
proprietary extensions. Extended metadata goes in the standard metadata  map or
companion files.
2. Framework-agnostic — No dependency on any agent framework. Provides traits;
frameworks implement them. Works with LangGraph, Strands, CrewAI, AutoAgents, or
bare-metal custom agents.
3. Cascade-by-default — Selection strategies compose in a fast→slow cascade: regex
triggers (µs) → semantic similarity (ms) → LLM classification (s). Each layer is optional.
4. Enforcement is code, not prompt — Guardrails are deterministic hook functions
executed at the framework level. The LLM cannot override a cancelled skill call.
5. Closed-loop learning — Every selection decision is logged. Trigger tests run as CI.
Description optimization is data-driven.
6. Multilingual-first — Chinese + English are first-class. Default embedding model
supports 100+ languages.
7. Zero required network — Default configuration runs fully offline (local embedding
model, local skill files). API embeddings and remote registries are opt-in.
3. Workspace Structure
agent-skill-engine/
├── Cargo.toml                      # Workspace root
├── crates/
│   ├── skm-core/                   # Skill schema, parser, registry
│   ├── skm-select/                 # Multi-strategy selection engine
│   ├── skm-embed/                  # Embedding abstraction + providers
│   ├── skm-disclose/               # Progressive disclosure / context management
│   ├── skm-enforce/                # Execution guardrails & hooks
│   ├── skm-learn/                  # Evaluation, metrics, optimization
│   ├── skm-cli/                    # Developer CLI (`skm`)
│   └── ase/                        # Facade crate re-exporting everything
├── models/                         # Model weight download scripts
├── tests/                          # Integration tests
├── benches/                        # Criterion benchmarks
└── examples/                       # Usage examples
Crate Dependency Graph
ase (facade)
 ├── skm-core
 ├── skm-select ──→ skm-core, skm-embed, skm-disclose
 ├── skm-embed
 ├── skm-disclose ──→ skm-core
 ├── skm-enforce ──→ skm-core
 ├── skm-learn ──→ skm-core, skm-select
 └── skm-cli ──→ all of the above
4. Crate: skm-core
4.1 Skill Schema
Fully compatible with the agentskills.io specification. Parses SKILL.md frontmatter + body.
/// A parsed Agent Skill, compatible with agentskills.io spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    // === Required fields (spec) ===
    pub name: SkillName,                    // 1-64 chars, lowercase + hyphens
    pub description: String,                // Free text, used for selection
    // === Optional fields (spec) ===
    pub license: Option<String>,
    pub compatibility: Option<String>,      // 1-500 chars
    pub metadata: HashMap<String, String>,  // Extensible key-value pairs
    // === Body ===
    pub instructions: String,               // Markdown body after frontmatter
    pub source_path: PathBuf,               // Filesystem path to SKILL.md
    // === Extended metadata (stored in `metadata` map, not custom fields) ===
    // metadata.triggers       — comma-separated trigger patterns
    // metadata.examples       — JSON array of {input, expected_skill} pairs
    // metadata.allowed_tools  — comma-separated tool names
    // metadata.tags           — comma-separated category tags
}
/// Validated skill name: 1-64 chars, allowed chars: [a-zA-Z0-9._-]
/// Stored as lowercase internally for case-insensitive matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillName(String);
impl SkillName {
    /// Validates charset [a-zA-Z0-9._-], 1-64 chars. Stores lowercase.
    pub fn new(s: &str) -> Result<Self, ValidationError> { /* ... */ }
    /// Raw string value (always lowercase).
    pub fn as_str(&self) -> &str { &self.0 }
}
4.2 Skill Parser
4.3 Skill Registry
In-memory registry with filesystem watching.
pub struct SkillRegistry {
    skills: HashMap<SkillName, SkillEntry>,
    directories: Vec<PathBuf>,
    watcher: Option<RecommendedWatcher>,  // notify crate
}
/// Internal entry tracking loading state.
pub struct SkillParser {
    strict: bool, // If true, reject skills with invalid frontmatter
}
impl SkillParser {
    /// Parse a SKILL.md file into a Skill struct.
    pub fn parse_file(&self, path: &Path) -> Result<Skill, ParseError>;
    /// Parse SKILL.md content from a string.
    pub fn parse_str(&self, content: &str) -> Result<Skill, ParseError>;
    /// Parse only frontmatter (name + description). Used for catalog-level loadi
    pub fn parse_metadata(&self, path: &Path) -> Result<SkillMetadata, ParseError
}
/// Lightweight metadata-only view (for progressive disclosure Level 0).
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub name: SkillName,
    pub description: String,
    pub tags: Vec<String>,
    pub triggers: Vec<String>,
    pub source_path: PathBuf,
    pub content_hash: u64,           // For cache invalidation
    pub estimated_tokens: usize,     // Estimated token count of full body
}
struct SkillEntry {
    metadata: SkillMetadata,                    // Always loaded
    full: tokio::sync::OnceCell<Skill>,         // Lazily loaded on first activation (async)
    embeddings: tokio::sync::OnceCell<SkillEmbeddings>, // Lazily computed (async)
    stats: SkillStats,                          // Selection/usage counters
}
impl SkillRegistry {
    /// Create a new registry scanning the given directories.
    pub async fn new(dirs: &[PathBuf]) -> Result<Self>;
    /// Reload skills from disk. Called on filesystem change or manually.
    pub async fn refresh(&mut self) -> Result<RefreshReport>;
    /// Get metadata for all registered skills.
    pub fn catalog(&self) -> Vec<&SkillMetadata>;
    /// Get a fully-loaded skill by name (triggers progressive loading).
    pub async fn get(&self, name: &SkillName) -> Result<&Skill>;
    /// Register a skill programmatically (not from filesystem).
    pub fn register(&mut self, skill: Skill) -> Result<()>;
    /// Deregister a skill.
    pub fn deregister(&mut self, name: &SkillName) -> Result<()>;
    /// Number of registered skills.
    pub fn len(&self) -> usize;
}
4.4 Error Types
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid skill name: {0}")]
    InvalidName(String),
    #[error("Parse error in {path}: {reason}")]
    Parse { path: PathBuf, reason: String },
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    #[error("Skill not found: {0}")]
    NotFound(SkillName),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Duplicate skill name: {0}")]
    Duplicate(SkillName),
}
5. Crate: skm-embed
Embedding abstraction layer. Trait-based, pluggable backends.
5.1 Trait Definition
/// Core embedding provider trait.
/// All implementations must be Send + Sync for use across async boundaries.
/// Core embedding provider trait.
/// All implementations must be Send + Sync for use across async boundaries.
/// `embed` is async to support both local inference (via spawn_blocking)
/// and API-based providers (network IO).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embeddings for a batch of texts.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError>;
    /// Vector dimensionality of this provider.
    fn dimensions(&self) -> usize;
    /// Human-readable model identifier.
    fn model_id(&self) -> &str;
}
/// A single embedding vector with metadata.
#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,
    pub text_hash: u64,             // For cache lookups
}
impl Embedding {
    /// Cosine similarity with another embedding.
    pub fn cosine_similarity(&self, other: &Embedding) -> f32;
    /// Dot product (for pre-normalized vectors).
    pub fn dot_product(&self, other: &Embedding) -> f32;
}
5.2 Local Providers
// ── Feature: embed-bge-m3 (default) ─────────────────────
/// BGE-M3: 568M params, 100+ languages, 1024-dim.
/// Chinese + English first-class support.
/// ONNX quantized: ~571MB, ~15ms/query on CPU.
pub struct BgeM3Provider {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
    cache: LruCache<u64, Vec<f32>>,
}
// ── Feature: embed-qwen3 ────────────────────────────────
/// Qwen3-Embedding-0.6B: 600M params, best Chinese accuracy.
/// Candle backend. Matryoshka: 32-1024 dims.
pub struct Qwen3Provider {
    model: candle_transformers::models::qwen3::Qwen3Model,
    tokenizer: tokenizers::Tokenizer,
    output_dims: usize,             // Configurable via Matryoshka
    cache: LruCache<u64, Vec<f32>>,
}
// ── Feature: embed-gte ──────────────────────────────────
/// gte-multilingual-base: 305M params, 70+ languages, 768-dim.
/// Lightest multilingual option.
pub struct GteMultilingualProvider {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
    cache: LruCache<u64, Vec<f32>>,
}
// ── Feature: embed-minilm ───────────────────────────────
/// all-MiniLM-L6-v2: 22M params, English only, 384-dim.
/// ~22MB, <5ms/query. For English-only lightweight deployments.
pub struct MiniLmProvider {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
    cache: LruCache<u64, Vec<f32>>,
}
5.3 API Providers
// ── Feature: embed-api ──────────────────────────────────
/// OpenAI text-embedding-3-small / text-embedding-3-large.
pub struct OpenAiEmbedProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    cache: LruCache<u64, Vec<f32>>,
}
/// Cohere embed-v4 / embed-multilingual-v3.
pub struct CohereEmbedProvider { /* ... */ }
/// Any OpenAI-compatible API (Ollama, vLLM, LiteLLM, etc.)
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    cache: LruCache<u64, Vec<f32>>,
}
5.4 Multi-Component Embeddings
Skill descriptions are decomposed into semantic components before embedding, following
the research showing this dramatically improves matching accuracy.
/// Multi-component embedding for a single skill.
/// Each component is embedded separately and scored with configurable weights.
#[derive(Debug, Clone)]
pub struct SkillEmbeddings {
    pub skill_name: SkillName,
    pub description: Embedding,     // Main description text
    pub triggers: Embedding,        // Trigger keywords concatenated
    pub tags: Embedding,            // Tags concatenated
    pub examples: Embedding,        // Example inputs concatenated
    pub weights: ComponentWeights,
}
#[derive(Debug, Clone)]
pub struct ComponentWeights {
    pub description: f32,           // Default: 0.45
    pub triggers: f32,              // Default: 0.25
    pub tags: f32,                  // Default: 0.15
    pub examples: f32,              // Default: 0.15
}
impl SkillEmbeddings {
    /// Compute weighted similarity score against a query embedding.
    pub fn score(&self, query: &Embedding) -> f32 {
        self.weights.description * self.description.cosine_similarity(query)
        + self.weights.triggers * self.triggers.cosine_similarity(query)
        + self.weights.tags * self.tags.cosine_similarity(query)
        + self.weights.examples * self.examples.cosine_similarity(query)
    }
}
5.5 Embedding Index
Pre-computed, serializable index for fast startup.
/// Persistent embedding index. Serialized to disk via bincode.
/// Rebuilds only when skill content changes (tracked via content_hash).
pub struct EmbeddingIndex {
    entries: Vec<SkillEmbeddings>,
    model_id: String,               // Invalidate if model changes
    built_at: SystemTime,
}
impl EmbeddingIndex {
    /// Build index from registry + provider. Parallelized.
    pub async fn build(
        registry: &SkillRegistry,
        provider: &dyn EmbeddingProvider,
        weights: ComponentWeights,
    ) -> Result<Self>;
    /// Load from disk cache. Returns None if stale.
    pub fn load_cached(path: &Path, registry: &SkillRegistry) -> Result<Option<Se
    /// Save to disk.
    pub fn save(&self, path: &Path) -> Result<()>;
    /// Query: return top-k skills by similarity.
    pub fn query(&self, query_embedding: &Embedding, top_k: usize) -> Vec<ScoredS
    /// Query with adaptive k: return skills above threshold,
    /// with automatic gap detection.
    pub fn query_adaptive(
        &self,
        query_embedding: &Embedding,
        min_score: f32,
        max_k: usize,
        gap_threshold: f32,          // Score gap that triggers cutoff
    ) -> Vec<ScoredSkill>;
}
#[derive(Debug, Clone)]
5.6 Performance Optimizations
/// SIMD-accelerated vector operations.
/// Uses std::simd when available, falls back to scalar.
pub(crate) mod simd {
    /// Dot product of two f32 slices. Loop-unrolled for pipeline efficiency.
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32;
    /// L2 normalize a vector in-place.
    pub fn normalize(v: &mut [f32]);
    /// Batch cosine similarity: one query vs N candidates.
    /// Returns scores in the same order as candidates.
    pub fn batch_cosine(query: &[f32], candidates: &[&[f32]]) -> Vec<f32>;
}
/// Top-K selection. Uses partial sort for small K, heap for large K.
pub(crate) fn top_k<T: Ord>(items: &mut [(f32, T)], k: usize) -> &[(f32, T)];
5.7 Feature Flags
pub struct ScoredSkill {
    pub name: SkillName,
    pub score: f32,
    pub component_scores: ComponentScores, // Per-component breakdown
}
#[derive(Debug, Clone)]
pub struct ComponentScores {
    pub description: f32,
    pub triggers: f32,
    pub tags: f32,
    pub examples: f32,
}
[features]
default = ["embed-bge-m3"]
# Local models (choose one or more)
embed-bge-m3   = ["dep:fastembed"]              # 571MB, 100+ langs ★ recommended
embed-qwen3    = ["dep:fastembed", "dep:candle-core", "dep:candle-nn"]  # Best zh
embed-gte      = ["dep:ort", "dep:tokenizers"]  # 305M, lightest multilingual
embed-minilm   = ["dep:fastembed"]              # 22MB, English only
6. Crate: skm-select
The core differentiator. Multi-strategy, cascading skill selection engine.
6.1 Selection Pipeline
6.2 Strategy Trait
/// A skill selection strategy. Strategies are composable.
/// `select` is async — Semantic and LLM strategies require async IO.
/// Trigger strategy uses trivial async (returns immediately).
#[async_trait]
pub trait SelectionStrategy: Send + Sync {
    /// Rank skills for a given query. Returns scored candidates.
    async fn select(
        &self,
        query: &str,
# API providers
embed-openai   = ["dep:reqwest", "dep:tokio"]
embed-cohere   = ["dep:reqwest", "dep:tokio"]
embed-compat   = ["dep:reqwest", "dep:tokio"]   # Any OpenAI-compatible API
# No embedding (trigger-only mode)
no-embed = []
User Query
    │
    ▼
┌──────────────┐    µs     ┌─────────────────┐
│ Trigger Match │──────────▶│ Exact match?    │──▶ Return immediately
│ (regex/kw)   │           │ Confident?      │
└──────────────┘           └────────┬────────┘
                                    │ No confident match
                                    ▼
                           ┌─────────────────┐
                           │ Semantic Search  │    ms
                           │ (embedding sim)  │──▶ Score above threshold? → Retur
                           └────────┬────────┘
                                    │ Ambiguous / below threshold
                                    ▼
                           ┌─────────────────┐
                           │ LLM Classifier  │    s
                           │ (intent routing) │──▶ Return with explanation
                           └─────────────────┘
        candidates: &[&SkillMetadata],
        ctx: &SelectionContext,
    ) -> Result<Vec<SelectionResult>, SelectError>;
    /// Strategy name for logging/metrics.
    fn name(&self) -> &str;
    /// Expected latency class.
    fn latency_class(&self) -> LatencyClass;
}
#[derive(Debug, Clone, Copy)]
pub enum LatencyClass {
    Microseconds,   // Trigger matching
    Milliseconds,   // Semantic search
    Seconds,        // LLM classification
}
#[derive(Debug, Clone)]
pub struct SelectionResult {
    pub skill: SkillName,
    pub score: f32,                 // 0.0 - 1.0 normalized
    pub confidence: Confidence,
    pub strategy: String,           // Which strategy produced this
    pub reasoning: Option<String>,  // LLM strategies can explain
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Confidence {
    Definite,     // Single clear match, proceed without fallback
    High,         // Strong match, no need for slower strategies
    Medium,       // Decent match, but slower strategy might do better
    Low,          // Weak signal, definitely try next strategy
    None,         // No match at all
}
/// Contextual information available during selection.
pub struct SelectionContext {
    pub conversation_history: Vec<String>,  // Recent messages for context
    pub active_skills: Vec<SkillName>,      // Currently loaded skills
    pub user_locale: Option<String>,        // e.g., "zh-CN", "en-US"
    pub custom: HashMap<String, String>,    // Framework-specific context
}
6.3 Built-in Strategies
6.3.1 Trigger Strategy (µs)
/// Fast-path matching using patterns defined in skill metadata.
/// Supports: exact keywords, regex patterns, glob patterns.
pub struct TriggerStrategy {
    matchers: Vec<SkillMatcher>,
}
struct SkillMatcher {
    skill_name: SkillName,
    keywords: Vec<String>,          // Case-insensitive exact match
    patterns: Vec<Regex>,           // Compiled regex patterns
    negative_patterns: Vec<Regex>,  // Patterns that should NOT trigger
}
impl TriggerStrategy {
    /// Build from registry. Extracts `metadata.triggers` from each skill.
    pub fn from_registry(registry: &SkillRegistry) -> Result<Self>;
}
Trigger format in SKILL.md metadata:
metadata:
  triggers: "pdf, .pdf, extract text from pdf, merge pdf"
  negative-triggers: "google docs, spreadsheet"
6.3.2 Semantic Strategy (ms)
/// Embedding-based semantic similarity search.
pub struct SemanticStrategy {
    provider: Arc<dyn EmbeddingProvider>,
    index: Arc<RwLock<EmbeddingIndex>>,
    config: SemanticConfig,
}
pub struct SemanticConfig {
    pub top_k: usize,               // Max candidates to return (default: 5)
    pub min_score: f32,              // Minimum similarity threshold (default: 0.
    pub gap_threshold: f32,          // Score gap for adaptive cutoff (default: 0
    pub component_weights: ComponentWeights,
    pub use_adaptive_k: bool,        // Enable gap-based adaptive selection (defa
}
6.3.3 LLM Strategy (s)
The LLM strategy constructs a structured prompt:
Given these available skills:
1. pdf-processing: Extract text and tables from PDF files...
2. docx-creation: Create Word documents with formatting...
...
User query: "{query}"
Which skill(s) should handle this query? Respond in JSON:
{"skills": [{"name": "...", "confidence": 0.0-1.0, "reasoning": "..."}]}
6.3.4 Few-Shot Enhanced Strategy
/// Wraps any strategy with dynamic few-shot example injection.
/// Selects examples semantically similar to the current query.
pub struct FewShotEnhanced<S: SelectionStrategy> {
    inner: S,
    example_index: EmbeddingIndex,   // Index over example inputs
    examples: Vec<FewShotExample>,
    top_k_examples: usize,          // Default: 3
}
/// LLM-based intent classification for ambiguous queries.
pub struct LlmStrategy {
    client: Arc<dyn LlmClient>,
    config: LlmStrategyConfig,
}
/// LLM client trait — users implement for their LLM provider.
/// Both methods are async (all LLM calls are network IO).
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a prompt, receive a text response.
    async fn complete(&self, prompt: &str, max_tokens: usize) -> Result<String, LlmError>;
    /// Send a prompt with structured output (JSON mode).
    /// Default implementation calls complete() and parses JSON.
    async fn complete_structured(
        &self,
        prompt: &str,
        schema: &serde_json::Value,
        max_tokens: usize,
    ) -> Result<serde_json::Value, LlmError> {
        let text = self.complete(prompt, max_tokens).await?;
        serde_json::from_str(&text).map_err(|e| LlmError::ParseError(e.to_string()))
    }
}
pub struct LlmStrategyConfig {
    pub system_prompt: String,       // Default provided, customizable
    pub include_few_shot: bool,      // Include examples from skill metadata
    pub max_candidates: usize,       // Max skills to include in prompt (default:
    pub temperature: f32,            // Default: 0.0 (deterministic)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FewShotExample {
    pub input: String,              // Example user query
    pub expected_skill: SkillName,  // Correct skill for this query
    pub reasoning: Option<String>,  // Why this skill is correct
}
6.4 Cascade Selector
The main entry point. Composes strategies in a cascade.
/// Cascading skill selector. Tries strategies in order,
/// stopping when confidence is high enough.
pub struct CascadeSelector {
    strategies: Vec<(Box<dyn SelectionStrategy>, Confidence)>,
    // Each entry: (strategy, min_confidence_to_stop)
    config: CascadeConfig,
    metrics: Arc<SelectionMetrics>,
}
pub struct CascadeConfig {
    /// If true, always run all strategies and merge results.
    /// If false (default), stop at first confident result.
    pub exhaustive: bool,
    /// Maximum total latency budget. Skip slow strategies if exceeded.
    pub timeout: Duration,
    /// How to merge results from multiple strategies.
    pub merge_strategy: MergeStrategy,
}
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Take the highest-scoring result across all strategies.
    MaxScore,
    /// Weighted average of scores, with strategy-level weights.
    WeightedAverage,
    /// Reciprocal Rank Fusion across strategy rankings.
    RRF { k: f32 },
}
impl CascadeSelector {
    pub fn builder() -> CascadeSelectorBuilder;
    /// Select the best skill(s) for a query.
    pub async fn select(
        &self,
        query: &str,
        registry: &SkillRegistry,
        ctx: &SelectionContext,
    ) -> Result<SelectionOutcome>;
}
/// Complete outcome of a selection, with full audit trail.
#[derive(Debug)]
pub struct SelectionOutcome {
    pub selected: Vec<SelectionResult>,      // Final ranked results
    pub strategies_used: Vec<String>,        // Which strategies ran
    pub total_latency: Duration,
    pub per_strategy_latency: Vec<(String, Duration)>,
    pub fallback_used: bool,                 // Did we reach LLM fallback?
}
/// Builder for ergonomic cascade construction.
pub struct CascadeSelectorBuilder { /* ... */ }
impl CascadeSelectorBuilder {
    /// Add trigger strategy as first cascade level.
    /// Stops cascade if confidence >= Confidence::High.
    pub fn with_triggers(self) -> Self;
    /// Add semantic strategy as second cascade level.
    pub fn with_semantic(
        self,
        provider: Arc<dyn EmbeddingProvider>,
        config: SemanticConfig,
    ) -> Self;
    /// Add LLM strategy as final fallback.
    pub fn with_llm(
        self,
        client: Arc<dyn LlmClient>,
        config: LlmStrategyConfig,
    ) -> Self;
    /// Add a custom strategy at a specific cascade position.
    pub fn with_custom(
        self,
        strategy: Box<dyn SelectionStrategy>,
        stop_confidence: Confidence,
    ) -> Self;
    /// Set the cascade config.
    pub fn config(self, config: CascadeConfig) -> Self;
    /// Build the CascadeSelector.
    pub fn build(self, registry: &SkillRegistry) -> Result<CascadeSelector>;
}
Usage:
7. Crate: skm-disclose
Progressive disclosure — manages what’s loaded into the LLM context window.
7.1 Disclosure Levels
7.2 Context Manager
let selector = CascadeSelector::builder()
    .with_triggers()
    .with_semantic(embed_provider, SemanticConfig::default())
    .with_llm(llm_client, LlmStrategyConfig::default())
    .build(&registry)?;
let outcome = selector.select("merge these two PDFs", &registry, &ctx).await?;
// outcome.selected[0].skill == "pdf-processing"
// outcome.strategies_used == ["trigger"]  ← stopped early, "pdf" keyword matched
Level 0: Catalog     ~30-50 tokens/skill    Name + description only
Level 1: Activation  ~2000-5000 tokens      Full SKILL.md body loaded
Level 2: References  Variable               scripts/, references/ loaded on deman
/// Manages what skill content is loaded into the LLM's context window.
pub struct ContextManager {
    budget: TokenBudget,
    loaded: Vec<LoadedSkill>,
}
pub struct TokenBudget {
    pub max_tokens: usize,           // Total budget for skills in context
    pub catalog_reserve: usize,      // Reserved for Level 0 catalog
    pub per_skill_max: usize,        // Max tokens per activated skill
}
#[derive(Debug)]
pub struct LoadedSkill {
    pub name: SkillName,
    pub level: DisclosureLevel,
    pub tokens_used: usize,
    pub loaded_files: Vec<PathBuf>,  // Which reference files are loaded
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisclosureLevel {
    Catalog,     // Level 0
    Activated,   // Level 1
    Referenced,  // Level 2 (specific files)
}
impl ContextManager {
    pub fn new(budget: TokenBudget) -> Self;
    /// Generate the Level 0 catalog string for the system prompt.
    /// Only name + description for each skill.
    pub fn catalog_prompt(&self, registry: &SkillRegistry) -> String;
    /// Activate a skill: load full SKILL.md into context.
    /// Returns the content to inject into the LLM prompt.
    pub async fn activate(
        &mut self,
        name: &SkillName,
        registry: &SkillRegistry,
    ) -> Result<ActivationPayload>;
    /// Load a specific reference file from an activated skill.
    pub async fn load_reference(
        &mut self,
        skill: &SkillName,
        file: &str,  // Relative path, e.g., "references/FORMS.md"
        registry: &SkillRegistry,
    ) -> Result<String>;
    /// Deactivate a skill, freeing context budget.
    pub fn deactivate(&mut self, name: &SkillName);
    /// Current token usage.
    pub fn tokens_used(&self) -> usize;
    /// Remaining budget.
    pub fn tokens_remaining(&self) -> usize;
7.3 Token Estimator
8. Crate: skm-enforce
Deterministic execution guardrails. The LLM cannot override these.
8.1 Hook System
/// Hook that runs BEFORE a skill is activated.
/// Can inspect, modify, or cancel the activation.
pub trait BeforeSkillActivation: Send + Sync {
    fn before_activate(
        &self,
        skill: &SkillName,
        query: &str,
        ctx: &EnforcementContext,
    ) -> HookDecision;
}
/// Hook that runs AFTER a skill produces output.
/// Can inspect, modify, or reject the output.
}
#[derive(Debug)]
pub struct ActivationPayload {
    pub skill_name: SkillName,
    pub instructions: String,        // Full SKILL.md body
    pub available_references: Vec<String>, // Files that can be loaded on demand
    pub tokens: usize,
}
/// Fast token count estimator. Not exact, but good enough for budgeting.
/// Uses the heuristic: ~4 chars per token for English, ~2 chars for Chinese.
pub struct TokenEstimator {
    chars_per_token: f32,            // Configurable, default 3.5
}
impl TokenEstimator {
    pub fn estimate(&self, text: &str) -> usize;
    pub fn estimate_cjk_aware(&self, text: &str) -> usize; // Better for mixed zh
}
pub trait AfterSkillExecution: Send + Sync {
    fn after_execute(
        &self,
        skill: &SkillName,
        output: &str,
        ctx: &EnforcementContext,
    ) -> HookDecision;
}
/// Decision returned by a hook.
#[derive(Debug)]
pub enum HookDecision {
    /// Allow the operation to proceed.
    Allow,
    /// Allow, but modify the input/output.
    Modify(String),
    /// Cancel the operation with a reason.
    /// The LLM receives this message instead of executing.
    Cancel {
        reason: String,
        suggest_alternative: Option<SkillName>,
    },
    /// Require human approval before proceeding.
    RequireApproval {
        reason: String,
        timeout: Duration,
    },
}
/// Context available to enforcement hooks.
pub struct EnforcementContext {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub conversation_history: Vec<String>,
    pub active_policies: Vec<String>,
    pub custom: HashMap<String, serde_json::Value>,
}
8.2 Policy Engine
/// Declarative policy rules for skill access control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub rules: Vec<PolicyRule>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub skill_pattern: String,       // Glob pattern matching skill names
    pub action: PolicyAction,
    pub conditions: Vec<Condition>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    Allow,
    Deny { reason: String },
    RequireApproval,
    RateLimit { max_per_minute: u32 },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    UserIn(Vec<String>),
    UserNotIn(Vec<String>),
    TimeWindow { start: String, end: String },
    SkillTagIs(String),
    SkillTagNot(String),
    Custom { key: String, value: String },
}
/// Policy engine evaluates rules against context.
pub struct PolicyEngine {
    policies: Vec<Policy>,
}
impl PolicyEngine {
    pub fn new(policies: Vec<Policy>) -> Self;
    /// Load policies from a YAML/JSON file.
    pub fn from_file(path: &Path) -> Result<Self>;
    /// Evaluate whether a skill activation is allowed.
    pub fn evaluate(
        &self,
        skill: &SkillName,
        ctx: &EnforcementContext,
    ) -> PolicyDecision;
}
#[derive(Debug)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub matched_rules: Vec<String>,
    pub reason: Option<String>,
}
8.3 Output Validator
/// Validates skill execution output against expected schemas.
pub struct OutputValidator {
    validators: HashMap<SkillName, Box<dyn Validator>>,
}
pub trait Validator: Send + Sync {
    fn validate(&self, output: &str) -> ValidationResult;
}
pub enum ValidationResult {
    Valid,
    Invalid { errors: Vec<String> },
    Repaired { output: String, warnings: Vec<String> },
}
// Built-in validators
pub struct JsonSchemaValidator { schema: serde_json::Value }
pub struct RegexValidator { pattern: Regex }
pub struct LengthValidator { min: usize, max: usize }
8.4 Enforcement Pipeline
/// Complete enforcement pipeline combining hooks, policies, and validators.
pub struct EnforcementPipeline {
    before_hooks: Vec<Box<dyn BeforeSkillActivation>>,
    after_hooks: Vec<Box<dyn AfterSkillExecution>>,
    policy_engine: PolicyEngine,
    output_validators: OutputValidator,
    audit_log: Arc<dyn AuditLog>,
}
/// Audit log trait for compliance tracking.
pub trait AuditLog: Send + Sync {
    fn log_activation(&self, entry: ActivationAuditEntry);
    fn log_enforcement(&self, entry: EnforcementAuditEntry);
}
impl EnforcementPipeline {
    pub fn builder() -> EnforcementPipelineBuilder;
    /// Run pre-activation checks.
    pub async fn check_before(
        &self,
        skill: &SkillName,
        query: &str,
        ctx: &EnforcementContext,
    ) -> Result<HookDecision>;
    /// Run post-execution checks.
    pub async fn check_after(
        &self,
        skill: &SkillName,
        output: &str,
        ctx: &EnforcementContext,
    ) -> Result<HookDecision>;
}
9. Crate: skm-learn
Closed-loop optimization: test, measure, improve.
9.1 Trigger Test Harness
/// Evaluation harness for testing skill selection accuracy.
/// Inspired by the agentskills.io description optimization methodology.
pub struct TriggerTestHarness {
    selector: Arc<CascadeSelector>,
    registry: Arc<SkillRegistry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub query: String,
    pub expected: TestExpectation,
    pub runs: usize,                 // How many times to run (default: 3)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestExpectation {
    /// This query SHOULD trigger the named skill.
    ShouldTrigger(SkillName),
    /// This query SHOULD NOT trigger the named skill.
    ShouldNotTrigger(SkillName),
    /// This query should trigger one of these skills.
    ShouldTriggerOneOf(Vec<SkillName>),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    pub name: String,
    pub cases: Vec<TestCase>,
}
impl TriggerTestHarness {
    pub async fn run_suite(&self, suite: &TestSuite) -> TestReport;
    pub async fn run_case(&self, case: &TestCase) -> TestCaseResult;
}
#[derive(Debug, Serialize)]
pub struct TestReport {
    pub suite_name: String,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f32,
    pub per_skill_results: HashMap<SkillName, SkillTestReport>,
    pub cases: Vec<TestCaseResult>,
    pub timestamp: SystemTime,
}
#[derive(Debug, Serialize)]
pub struct TestCaseResult {
    pub query: String,
    pub expectation: TestExpectation,
    pub actual_selections: Vec<SelectionResult>,
    pub trigger_rate: f32,           // Fraction of runs that matched
    pub passed: bool,
    pub latency: Duration,
}
#[derive(Debug, Serialize)]
pub struct SkillTestReport {
    pub skill: SkillName,
    pub should_trigger_pass_rate: f32,
    pub should_not_trigger_pass_rate: f32,
    pub precision: f32,
    pub recall: f32,
    pub f1: f32,
}
Test suite file format (YAML):
name: pdf-processing-tests
cases:
  - query: "Extract text from this PDF"
    expected: { should_trigger: pdf-processing }
  - query: "Merge these two PDFs"
    expected: { should_trigger: pdf-processing }
  - query: "Create a Word document"
    expected: { should_not_trigger: pdf-processing }
  - query: "Convert this to a spreadsheet"
    expected: { should_not_trigger: pdf-processing }
  - query: "这个PDF⾥的表格提取出来"
    expected: { should_trigger: pdf-processing }
9.2 Selection Metrics
/// Real-time metrics collector for selection performance.
pub struct SelectionMetrics {
    // Per-strategy metrics
    strategy_calls: HashMap<String, AtomicU64>,
    strategy_latency: HashMap<String, Histogram>,
    // Per-skill metrics
    skill_selections: HashMap<SkillName, AtomicU64>,
    skill_rejections: HashMap<SkillName, AtomicU64>,
    // Cascade metrics
    cascade_depth: Histogram,       // How many strategies were needed
    fallback_rate: AtomicU64,       // How often LLM fallback was used
    total_queries: AtomicU64,
    // Accuracy tracking (when ground truth is available)
    correct_selections: AtomicU64,
    incorrect_selections: AtomicU64,
}
impl SelectionMetrics {
    /// Export as Prometheus-compatible metrics.
    pub fn prometheus_export(&self) -> String;
    /// Export as JSON for dashboarding.
    pub fn to_json(&self) -> serde_json::Value;
    /// Summary statistics.
    pub fn summary(&self) -> MetricsSummary;
}
#[derive(Debug, Serialize)]
pub struct MetricsSummary {
    pub total_queries: u64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub trigger_hit_rate: f32,       // % resolved by triggers alone
    pub semantic_hit_rate: f32,      // % resolved by semantic (no LLM needed)
    pub llm_fallback_rate: f32,      // % that needed LLM
    pub most_selected_skills: Vec<(SkillName, u64)>,
    pub never_selected_skills: Vec<SkillName>,
}
9.3 Description Optimizer
/// Iteratively optimizes skill descriptions for better trigger accuracy.
/// Uses an LLM to generate candidate descriptions, then evaluates via test harne
pub struct DescriptionOptimizer {
    llm: Arc<dyn LlmClient>,
    harness: TriggerTestHarness,
    config: OptimizerConfig,
}
pub struct OptimizerConfig {
    pub max_iterations: usize,       // Default: 5
    pub max_description_length: usize, // Default: 500 chars
    pub improvement_threshold: f32,  // Stop if improvement < this (default: 0.02
}
impl DescriptionOptimizer {
    /// Optimize a single skill's description.
    /// Returns the best description found and the improvement delta.
    pub async fn optimize(
        &self,
        skill: &SkillName,
        test_suite: &TestSuite,
    ) -> Result<OptimizationResult>;
9.4 Usage Analytics
/// Tracks skill usage patterns over time for insights.
pub struct UsageAnalytics {
    store: Box<dyn AnalyticsStore>,
}
pub trait AnalyticsStore: Send + Sync {
    fn record_selection(&self, event: SelectionEvent);
    fn query_events(&self, filter: EventFilter) -> Vec<SelectionEvent>;
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionEvent {
    pub timestamp: SystemTime,
    pub query: String,
    pub selected_skill: Option<SkillName>,
    pub all_candidates: Vec<ScoredSkill>,
    pub strategy_used: String,
    pub latency: Duration,
    /// Optimize all skills in the registry.
    pub async fn optimize_all(
        &self,
        test_suites: &HashMap<SkillName, TestSuite>,
    ) -> Result<Vec<OptimizationResult>>;
}
#[derive(Debug, Serialize)]
pub struct OptimizationResult {
    pub skill: SkillName,
    pub original_description: String,
    pub optimized_description: String,
    pub original_pass_rate: f32,
    pub optimized_pass_rate: f32,
    pub iterations_used: usize,
    pub history: Vec<OptimizationIteration>,
}
#[derive(Debug, Serialize)]
pub struct OptimizationIteration {
    pub iteration: usize,
    pub description: String,
    pub pass_rate: f32,
    pub test_report: TestReport,
}
    pub user_feedback: Option<Feedback>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Feedback {
    Correct,
    Incorrect { expected: SkillName },
    Irrelevant,
}
// Built-in stores
pub struct SqliteAnalyticsStore { /* ... */ }  // Feature: analytics-sqlite
pub struct InMemoryAnalyticsStore { /* ... */ }
10. Crate: skm-cli
Developer CLI for the full skill lifecycle.
ase — Agent Skill Engine CLI
USAGE:
    ase <COMMAND>
COMMANDS:
    init          Create a new skill from template
    validate      Validate SKILL.md files
    list          List skills in a directory
    test          Run trigger test suites
    bench         Benchmark selection performance
    optimize      Optimize skill descriptions
    index         Build/rebuild embedding index
    select        Interactive skill selection (for debugging)
    stats         Show usage analytics
    serve         Start HTTP API for remote selection
    export        Export metrics (Prometheus/JSON)
EXAMPLES:
    # Create a new skill
    ase init my-new-skill --lang zh-en
    # Validate all skills in a directory
    ase validate ./skills/
    # Run trigger tests
10.1 HTTP API (via ase serve )
POST /v1/select
  Body: { "query": "...", "context": { ... } }
  Response: { "selected": [...], "latency_ms": 12 }
POST /v1/activate
  Body: { "skill": "pdf-processing" }
  Response: { "instructions": "...", "tokens": 3200 }
GET /v1/skills
  Response: { "skills": [{ "name": "...", "description": "..." }, ...] }
GET /v1/metrics
  Response: Prometheus text format
POST /v1/feedback
  Body: { "query": "...", "selected": "...", "feedback": "correct" }
11. Facade Crate: ase
Re-exports everything for simple single-dependency usage.
    ase test ./tests/trigger-suite.yaml --runs 5
    # Interactive selection mode (type queries, see which skills match)
    ase select --skills ./skills/ --strategy cascade
    # Benchmark selection latency
    ase bench --skills ./skills/ --queries ./tests/queries.txt --output bench.jso
    # Optimize a skill's description
    ase optimize pdf-processing --test-suite ./tests/pdf-tests.yaml --llm openai
    # Build embedding index
    ase index --skills ./skills/ --model bge-m3 --output ./cache/index.bin
    # Show analytics dashboard
    ase stats --db ./analytics.sqlite --top 20
    # Start HTTP selection API
    ase serve --skills ./skills/ --port 8080
// In user's Cargo.toml:
// [dependencies]
// ase = { version = "0.1", features = ["embed-bge-m3"] }
pub use skm_core::*;
pub use skm_select::*;
pub use skm_embed::*;
pub use skm_disclose::*;
pub use skm_enforce::*;
pub use skm_learn::*;
Quick start:
use ase::prelude::*;
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Build registry
    let registry = SkillRegistry::new(&["./skills"]).await?;
    // 2. Build embedding provider (BGE-M3 by default)
    let embedder = BgeM3Provider::new()?;
    // 3. Build selection engine
    let selector = CascadeSelector::builder()
        .with_triggers()
        .with_semantic(Arc::new(embedder), SemanticConfig::default())
        .build(&registry)?;
    // 4. Select a skill
    let outcome = selector.select(
        "Extract tables from this PDF",
        &registry,
        &SelectionContext::default(),
    ).await?;
    println!("Selected: {:?}", outcome.selected);
    // 5. Progressive disclosure
    let mut ctx_mgr = ContextManager::new(TokenBudget::default());
    let payload = ctx_mgr.activate(&outcome.selected[0].skill, &registry).await?;
    println!("Loaded {} tokens of instructions", payload.tokens);
    Ok(())
}
12. Feature Flag Summary
[workspace.features]
# Embedding models (choose at least one, or no-embed)
embed-bge-m3    # Default. 100+ langs, best zh/en balance. ~571MB quantized.
embed-qwen3     # Best zh/en accuracy. Candle backend. ~500MB.
embed-gte       # Lightest multilingual (305M). 70+ langs.
embed-minilm    # English only. 22MB. Fastest.
embed-openai    # OpenAI API.
embed-cohere    # Cohere API.
embed-compat    # Any OpenAI-compatible endpoint.
no-embed        # Trigger-only mode. No embedding dependency.
# Analytics storage
analytics-sqlite  # SQLite-backed analytics. Default.
analytics-memory  # In-memory only (no persistence).
# HTTP server
http-server       # Enable `ase serve` command. Adds axum dependency.
# CLI
cli               # Build the CLI binary.
13. Dependencies
Core dependencies (always included):
Crate
Purpose
Version
serde  + serde_json  +
serde_yaml
Serialization
1.x
thiserror
Error types
2.x
tracing
Structured logging
0.1
notify
Filesystem watching
7.x
bincode
Binary serialization (embedding index
cache)
1.x
lru
LRU cache
0.12
regex
Trigger pattern matching
1.x
tokio
Async runtime
1.x
async-trait
Async trait support
0.1
Conditional dependencies:
Crate
Feature
Purpose
fastembed
embed-bge-m3 , embed-minilm ,
embed-qwen3
Local embedding
inference
ort
embed-gte
ONNX Runtime for
custom models
candle-core  +
candle-nn
embed-qwen3
Candle ML framework
tokenizers
All local embed features
HuggingFace tokenizers
reqwest
embed-openai , embed-cohere ,
embed-compat
HTTP client for API calls
axum
http-server
HTTP server framework
rusqlite
analytics-sqlite
SQLite storage
clap
cli
CLI argument parsing
hdrhistogram
Always
Latency histograms
14. Testing Strategy
14.1 Unit Tests
Every public function has unit tests.
Embedding providers tested with mock vectors.
Policy engine tested with combinatorial rule sets.
14.2 Integration Tests
End-to-end cascade selection with real skills.
Filesystem watcher integration (create/modify/delete SKILL.md).
Embedding index build → serialize → reload → query.
14.3 Benchmarks (Criterion)
bench_trigger_match  — 100 skills, 1000 queries.
bench_semantic_search  — 100 skills, varying top-k.
bench_cascade_full  — Full cascade with all strategies.
bench_embedding_batch  — Batch embedding latency per model.
bench_simd_dot_product  — SIMD vs scalar dot product.
14.4 Golden Test Suite
Ship a tests/golden/  directory with:
20 example skills covering diverse domains.
200 test queries with ground-truth skill assignments.
Expected selection outcomes for regression testing.
15. Performance Targets
Operation
Target
Notes
Trigger match (100 skills)
< 50µs
Regex pre-compiled
Semantic search (100 skills)
< 15ms
Includes embedding + similarity
Embedding index build (100 skills)
< 5s
Parallelized, cached
Embedding index load (cached)
< 10ms
Bincode deserialization
Full cascade (trigger hit)
< 100µs
Early exit, no embedding needed
Full cascade (semantic hit)
< 20ms
Trigger miss → semantic
Full cascade (LLM fallback)
1-3s
Depends on LLM provider
SKILL.md parse
< 1ms
YAML frontmatter + markdown split
Memory per skill (catalog)
< 1KB
Name + description + metadata
Memory per skill (with embeddings)
< 20KB
4 × 1024-dim vectors
16. Roadmap
v0.1 — Foundation
skm-core : Parser, registry, filesystem watching
skm-select : Trigger strategy, cascade framework
skm-cli : init , validate , list , select
Golden test suite
v0.2 — Intelligence
skm-embed : BGE-M3 + MiniLM providers, multi-component embeddings, index
skm-select : Semantic strategy, adaptive-k
skm-disclose : Progressive disclosure, token budgeting
skm-cli : index , bench
v0.3 — Safety
skm-enforce : Hook system, policy engine, output validators
skm-cli : Policy file loading
v0.4 — Learning
skm-learn : Trigger test harness, metrics, usage analytics
skm-cli : test , stats
v0.5 — Optimization
skm-learn : Description optimizer
skm-embed : Qwen3, GTE, API providers
skm-select : LLM strategy, few-shot enhanced
skm-cli : optimize
v1.0 — Production
skm-cli : serve  (HTTP API)
Prometheus metrics export
Comprehensive documentation
Published to crates.io
Python bindings via PyO3 (separate crate: ase-python )

## Appendix: Design Review Notes (2026-03-30)

### Changes from initial design (review by Clawd):

1. **`EmbeddingProvider::embed` → async** — Local providers use `spawn_blocking` internally, API providers need network IO. Sync `embed()` would block the tokio runtime.

2. **`SkillName` validation relaxed** — Changed from "lowercase + hyphens only" to `[a-zA-Z0-9._-]` with internal lowercase storage. Real-world skill names use underscores, dots, mixed case.

3. **`LlmClient` trait → async + `complete_structured`** — All LLM calls are network IO. Added `complete_structured()` with default JSON parsing implementation for structured output support.

4. **`SkillEntry` uses `tokio::sync::OnceCell`** — Not `std::cell::OnceCell`. Async lazy loading requires the tokio variant.

5. **`SelectionStrategy::select` → async** — Semantic strategy calls `EmbeddingProvider::embed` (async), LLM strategy calls `LlmClient::complete` (async). Only TriggerStrategy is synchronous internally but wrapped in trivial async.

6. **GID: Added project → crate `part_of` edges** — 8 crate components now connected to project root.

7. **GID: `CascadeSelector` depends on `SelectionStrategy` trait only** — Not on semantic/llm strategies directly. Cascade accepts any strategy via the trait; semantic and llm are optional plugins. This matches the feature flag design (you can use `no-embed` with trigger-only cascade).

8. **GID: `task-embed-tests` depends on `task-embed-simd`** — SIMD functions must be tested.

9. **GID: `task-learn-analytics` depends on `task-core-schema`** — Not on `task-learn-metrics`. Analytics (event recording) and metrics (counters) are independent subsystems.

10. **GID: `task-disclose-context` depends on `task-disclose-tokens`** — ContextManager needs TokenEstimator for budget enforcement.
