"""Type stubs for SKM - Agent Skill Engine"""

from typing import Dict, List, Optional

class SkillMetadata:
    """Metadata for a skill (lightweight view)."""

    @property
    def name(self) -> str:
        """Skill name."""
        ...

    @property
    def description(self) -> str:
        """Skill description."""
        ...

    @property
    def triggers(self) -> List[str]:
        """Trigger patterns for fast matching."""
        ...

    @property
    def tags(self) -> List[str]:
        """Tags/categories."""
        ...

    @property
    def source_path(self) -> str:
        """Path to SKILL.md file."""
        ...

    @property
    def content_hash(self) -> int:
        """Content hash for cache invalidation."""
        ...

    @property
    def estimated_tokens(self) -> int:
        """Estimated token count."""
        ...


class SkillRegistry:
    """In-memory skill registry with lazy loading."""

    def __init__(self, paths: List[str]) -> None:
        """Create a new registry scanning the given directories.

        Args:
            paths: List of directory paths to scan for SKILL.md files.
        """
        ...

    def get(self, name: str) -> Optional[SkillMetadata]:
        """Get metadata for a skill by name."""
        ...

    def list(self) -> List[str]:
        """List all skill names."""
        ...

    def catalog(self) -> List[SkillMetadata]:
        """Get the full catalog of skill metadata."""
        ...

    def refresh(self) -> Dict[str, List[str]]:
        """Refresh skills from disk. Returns dict with 'added', 'updated', 'removed' keys."""
        ...

    def __len__(self) -> int:
        """Number of registered skills."""
        ...


class SelectionResult:
    """Result of a skill selection operation."""

    @property
    def skill(self) -> str:
        """Name of the selected skill."""
        ...

    @property
    def score(self) -> float:
        """Normalized score (0.0 - 1.0)."""
        ...

    @property
    def confidence(self) -> str:
        """Confidence level (None, Low, Medium, High, Definite)."""
        ...

    @property
    def strategy(self) -> str:
        """Which strategy produced this result."""
        ...

    @property
    def reasoning(self) -> Optional[str]:
        """Optional reasoning."""
        ...


class TriggerStrategy:
    """Fast trigger-based skill selection (µs latency)."""

    @staticmethod
    def from_registry(registry: SkillRegistry) -> "TriggerStrategy":
        """Create a TriggerStrategy from a SkillRegistry."""
        ...

    def select(self, query: str, registry: SkillRegistry) -> List[SelectionResult]:
        """Select skills matching the query using triggers."""
        ...


class CascadeSelector:
    """Cascading skill selector with multiple strategies."""

    def __init__(self, registry: SkillRegistry) -> None:
        """Create a CascadeSelector with trigger strategy.

        Args:
            registry: SkillRegistry to build triggers from.
        """
        ...

    def select(self, query: str, registry: SkillRegistry) -> List[SelectionResult]:
        """Select the best skills for a query.

        Returns a list of SelectionResult ordered by score.
        """
        ...

    def select_with_stats(self, query: str, registry: SkillRegistry) -> Dict[str, object]:
        """Get selection outcome with full audit trail.

        Returns dict with 'selected', 'strategies_used', 'total_latency_ms', 'fallback_used'.
        """
        ...


class BgeM3Provider:
    """BGE-M3 embedding provider (1024-dim, multilingual)."""

    def __init__(self, cache_size: int = 1000) -> None:
        """Create a new BGE-M3 provider.

        Args:
            cache_size: Number of embeddings to cache (default: 1000).
        """
        ...

    def embed(self, text: str) -> List[float]:
        """Embed a single text. Returns embedding vector."""
        ...

    def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Embed multiple texts. Returns list of embedding vectors."""
        ...

    @property
    def dimensions(self) -> int:
        """Vector dimensions (1024 for BGE-M3)."""
        ...

    @property
    def model_id(self) -> str:
        """Model identifier."""
        ...


class MiniLmProvider:
    """MiniLM embedding provider (384-dim, English only, fast)."""

    def __init__(self, cache_size: int = 1000) -> None:
        """Create a new MiniLM provider.

        Args:
            cache_size: Number of embeddings to cache (default: 1000).
        """
        ...

    def embed(self, text: str) -> List[float]:
        """Embed a single text. Returns embedding vector."""
        ...

    def embed_batch(self, texts: List[str]) -> List[List[float]]:
        """Embed multiple texts. Returns list of embedding vectors."""
        ...

    @property
    def dimensions(self) -> int:
        """Vector dimensions (384 for MiniLM)."""
        ...

    @property
    def model_id(self) -> str:
        """Model identifier."""
        ...


class HookDecision:
    """Hook decision for enforcement pipeline."""

    @property
    def decision_type(self) -> str:
        """Decision type (allow, cancel, modify, require_approval)."""
        ...

    @property
    def reason(self) -> Optional[str]:
        """Reason for the decision (if cancelled or requires approval)."""
        ...

    @property
    def modified_output(self) -> Optional[str]:
        """Modified output (if decision_type is 'modify')."""
        ...

    def is_allowed(self) -> bool:
        """Check if the decision allows the action."""
        ...

    def is_cancelled(self) -> bool:
        """Check if the decision cancels the action."""
        ...


class EnforcementPipeline:
    """Enforcement pipeline for pre/post skill execution checks."""

    def __init__(self) -> None:
        """Create a new enforcement pipeline (allow-all by default)."""
        ...

    def check_before(
        self,
        skill_name: str,
        query: str,
        user_id: Optional[str] = None,
        session_id: Optional[str] = None,
    ) -> HookDecision:
        """Run pre-activation checks.

        Args:
            skill_name: Name of the skill to check.
            query: The user query.
            user_id: Optional user identifier.
            session_id: Optional session identifier.
        """
        ...

    def check_after(
        self,
        skill_name: str,
        output: str,
        user_id: Optional[str] = None,
        session_id: Optional[str] = None,
    ) -> HookDecision:
        """Run post-execution checks.

        Args:
            skill_name: Name of the skill that was executed.
            output: The skill's output to check.
            user_id: Optional user identifier.
            session_id: Optional session identifier.
        """
        ...
