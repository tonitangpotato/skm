/**
 * @skm/core - Agent Skill Engine Node.js bindings
 *
 * NAPI-RS bindings for the SKM (Skill Manager) Rust library.
 * Provides high-performance skill registration, selection, and embedding.
 */

/**
 * Skill metadata (lightweight view without full instructions).
 */
export class JsSkillMetadata {
  /** Skill name (1-64 chars, lowercase). */
  get name(): string;
  /** Skill description. */
  get description(): string;
  /** Trigger patterns for fast matching. */
  get triggers(): string[];
  /** Tags/categories. */
  get tags(): string[];
  /** Filesystem path to SKILL.md. */
  get sourcePath(): string;
  /** Content hash for cache invalidation (hex string). */
  get contentHash(): string;
  /** Estimated token count. */
  get estimatedTokens(): number;
}

/**
 * Report from a registry refresh operation.
 */
export interface JsRefreshReport {
  /** Skills that were added. */
  added: string[];
  /** Skills that were updated. */
  updated: string[];
  /** Skills that were removed. */
  removed: string[];
  /** Errors encountered during refresh. */
  errors: JsRefreshError[];
}

/**
 * Error encountered during refresh.
 */
export interface JsRefreshError {
  /** File path that caused the error. */
  path: string;
  /** Error message. */
  message: string;
}

/**
 * In-memory skill registry with lazy loading.
 *
 * @example
 * ```typescript
 * const registry = await JsSkillRegistry.new(['./skills', '~/.openclaw/skills']);
 * const skill = await registry.get('pdf-processing');
 * console.log(skill?.description);
 * ```
 */
export class JsSkillRegistry {
  /**
   * Create a new registry scanning the given directories.
   * @param paths - Array of directory paths to scan for SKILL.md files
   */
  static new(paths: string[]): Promise<JsSkillRegistry>;

  /**
   * Get metadata for a skill by name.
   * @param name - Skill name to look up
   * @returns Skill metadata or null if not found
   */
  get(name: string): Promise<JsSkillMetadata | null>;

  /**
   * List all registered skills.
   * @returns Array of skill metadata
   */
  list(): Promise<JsSkillMetadata[]>;

  /**
   * Number of registered skills.
   */
  len(): Promise<number>;

  /**
   * Check if registry is empty.
   */
  isEmpty(): Promise<boolean>;

  /**
   * Refresh skills from disk.
   * @returns Report of changes
   */
  refresh(): Promise<JsRefreshReport>;
}

/**
 * Result of a skill selection.
 */
export interface JsSelectionResult {
  /** Selected skill name. */
  skillName: string;
  /** Normalized score (0.0 - 1.0). */
  score: number;
  /** Confidence level. */
  confidence: 'none' | 'low' | 'medium' | 'high' | 'definite';
  /** Which strategy produced this result. */
  strategy: string;
  /** Optional reasoning (for LLM strategies). */
  reasoning?: string;
}

/**
 * Configuration for the cascade selector.
 */
export interface JsCascadeConfig {
  /** If true, always run all strategies and merge results. */
  exhaustive?: boolean;
  /** Maximum timeout in milliseconds. */
  timeoutMs?: number;
  /** Use trigger-only mode (no semantic/LLM fallback). */
  triggerOnly?: boolean;
}

/**
 * Context for skill selection.
 */
export interface JsSelectionContext {
  /** Recent conversation history. */
  conversationHistory?: string[];
  /** User locale (e.g., "zh-CN", "en-US"). */
  userLocale?: string;
}

/**
 * Cascading skill selector with trigger-based fast path.
 *
 * Implements a multi-strategy selection cascade:
 * 1. Trigger matching (µs) - regex/keyword patterns
 * 2. Semantic search (ms) - embedding similarity
 * 3. LLM classification (s) - fallback for ambiguous cases
 *
 * @example
 * ```typescript
 * const selector = await JsCascadeSelector.new('./skills');
 * const results = await selector.select('convert this PDF to text');
 * console.log(results[0]?.skillName); // 'pdf-processing'
 * ```
 */
export class JsCascadeSelector {
  /**
   * Create a new cascade selector.
   * @param skillsDir - Directory containing SKILL.md files
   * @param config - Optional configuration
   */
  static new(skillsDir: string, config?: JsCascadeConfig): Promise<JsCascadeSelector>;

  /**
   * Select the best skill(s) for a query.
   * @param query - User query to match against skills
   * @returns Array of selection results, sorted by score descending
   */
  select(query: string): Promise<JsSelectionResult[]>;

  /**
   * Select with custom context (conversation history, locale, etc.).
   * @param query - User query
   * @param context - Selection context
   */
  selectWithContext(query: string, context: JsSelectionContext): Promise<JsSelectionResult[]>;
}

/**
 * BGE-M3 embedding provider (1024 dimensions, multilingual).
 *
 * Best for: multilingual content, semantic search across languages.
 * Model size: ~500MB (downloaded on first use)
 *
 * @example
 * ```typescript
 * const provider = JsBgeM3Provider.new();
 * const embedding = await provider.embed('Hello, world!');
 * console.log(embedding.length); // 1024
 * ```
 */
export class JsBgeM3Provider {
  /**
   * Create a new BGE-M3 provider.
   * Downloads the model on first use (~500MB).
   */
  static new(): JsBgeM3Provider;

  /**
   * Generate embedding for a single text.
   * @param text - Text to embed
   * @returns Float64 array of embeddings
   */
  embed(text: string): Promise<number[]>;

  /**
   * Generate embeddings for multiple texts.
   * @param texts - Array of texts to embed
   * @returns Array of Float64 arrays
   */
  embedBatch(texts: string[]): Promise<number[][]>;

  /** Get the embedding dimensions (1024). */
  get dimensions(): number;

  /** Get the model identifier. */
  get modelId(): string;
}

/**
 * MiniLM embedding provider (384 dimensions, English-optimized).
 *
 * Best for: English content, smaller footprint, faster inference.
 * Model size: ~80MB (downloaded on first use)
 *
 * @example
 * ```typescript
 * const provider = JsMiniLmProvider.new();
 * const embeddings = await provider.embedBatch(['hello', 'world']);
 * console.log(embeddings.length); // 2
 * console.log(embeddings[0].length); // 384
 * ```
 */
export class JsMiniLmProvider {
  /**
   * Create a new MiniLM provider.
   * Downloads the model on first use (~80MB).
   */
  static new(): JsMiniLmProvider;

  /**
   * Generate embedding for a single text.
   * @param text - Text to embed
   * @returns Float64 array of embeddings
   */
  embed(text: string): Promise<number[]>;

  /**
   * Generate embeddings for multiple texts.
   * @param texts - Array of texts to embed
   * @returns Array of Float64 arrays
   */
  embedBatch(texts: string[]): Promise<number[][]>;

  /** Get the embedding dimensions (384). */
  get dimensions(): number;

  /** Get the model identifier. */
  get modelId(): string;
}
