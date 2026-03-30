"""
SKM - Agent Skill Engine for Python

Fast skill selection, embedding, and enforcement for AI agents.
"""

from skm._skm_native import (
    # Core types
    SkillMetadata,
    SkillRegistry,
    # Selection
    SelectionResult,
    TriggerStrategy,
    CascadeSelector,
    # Enforcement
    HookDecision,
    EnforcementPipeline,
)

# Embedding providers (may not be available depending on features)
try:
    from skm._skm_native import BgeM3Provider
except ImportError:
    BgeM3Provider = None

try:
    from skm._skm_native import MiniLmProvider
except ImportError:
    MiniLmProvider = None

__all__ = [
    # Core
    "SkillMetadata",
    "SkillRegistry",
    # Selection
    "SelectionResult",
    "TriggerStrategy",
    "CascadeSelector",
    # Embedding
    "BgeM3Provider",
    "MiniLmProvider",
    # Enforcement
    "HookDecision",
    "EnforcementPipeline",
]

__version__ = "0.1.0"
