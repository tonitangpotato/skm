"""Basic tests for SKM Python bindings."""

import os
import pytest

# Get the path to test fixtures
FIXTURES_DIR = os.path.join(os.path.dirname(__file__), "fixtures")


class TestSkillRegistry:
    """Tests for SkillRegistry."""

    def test_create_registry(self):
        """Test creating a registry from a directory."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        assert len(registry) == 1

    def test_list_skills(self):
        """Test listing skill names."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        skills = registry.list()
        assert "test-skill" in skills

    def test_get_metadata(self):
        """Test getting skill metadata."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        meta = registry.get("test-skill")
        
        assert meta is not None
        assert meta.name == "test-skill"
        assert "test skill" in meta.description.lower()
        assert "test" in meta.triggers
        assert meta.estimated_tokens > 0

    def test_get_nonexistent(self):
        """Test getting a nonexistent skill."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        meta = registry.get("nonexistent-skill")
        
        assert meta is None

    def test_catalog(self):
        """Test getting full catalog."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        catalog = registry.catalog()
        
        assert len(catalog) == 1
        assert catalog[0].name == "test-skill"

    def test_refresh(self):
        """Test refreshing the registry."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        report = registry.refresh()
        
        assert "added" in report
        assert "updated" in report
        assert "removed" in report


class TestTriggerStrategy:
    """Tests for TriggerStrategy."""

    def test_create_from_registry(self):
        """Test creating TriggerStrategy from registry."""
        from skm import SkillRegistry, TriggerStrategy

        registry = SkillRegistry([FIXTURES_DIR])
        strategy = TriggerStrategy.from_registry(registry)
        
        assert strategy is not None

    def test_trigger_match(self):
        """Test trigger matching."""
        from skm import SkillRegistry, TriggerStrategy

        registry = SkillRegistry([FIXTURES_DIR])
        strategy = TriggerStrategy.from_registry(registry)
        
        results = strategy.select("run pytest", registry)
        
        assert len(results) > 0
        assert results[0].skill == "test-skill"
        assert results[0].score > 0

    def test_trigger_no_match(self):
        """Test trigger with no match."""
        from skm import SkillRegistry, TriggerStrategy

        registry = SkillRegistry([FIXTURES_DIR])
        strategy = TriggerStrategy.from_registry(registry)
        
        results = strategy.select("create a spreadsheet", registry)
        
        assert len(results) == 0


class TestCascadeSelector:
    """Tests for CascadeSelector."""

    def test_create_selector(self):
        """Test creating a CascadeSelector."""
        from skm import SkillRegistry, CascadeSelector

        registry = SkillRegistry([FIXTURES_DIR])
        selector = CascadeSelector(registry)
        
        assert selector is not None

    def test_select(self):
        """Test skill selection."""
        from skm import SkillRegistry, CascadeSelector

        registry = SkillRegistry([FIXTURES_DIR])
        selector = CascadeSelector(registry)
        
        results = selector.select("run the tests", registry)
        
        assert len(results) > 0
        assert results[0].skill == "test-skill"

    def test_select_with_stats(self):
        """Test selection with stats."""
        from skm import SkillRegistry, CascadeSelector

        registry = SkillRegistry([FIXTURES_DIR])
        selector = CascadeSelector(registry)
        
        outcome = selector.select_with_stats("test this", registry)
        
        assert "selected" in outcome
        assert "strategies_used" in outcome
        assert "total_latency_ms" in outcome
        assert "fallback_used" in outcome


class TestSelectionResult:
    """Tests for SelectionResult attributes."""

    def test_result_attributes(self):
        """Test SelectionResult has expected attributes."""
        from skm import SkillRegistry, CascadeSelector

        registry = SkillRegistry([FIXTURES_DIR])
        selector = CascadeSelector(registry)
        
        results = selector.select("run test", registry)
        
        assert len(results) > 0
        result = results[0]
        
        assert hasattr(result, "skill")
        assert hasattr(result, "score")
        assert hasattr(result, "confidence")
        assert hasattr(result, "strategy")
        assert hasattr(result, "reasoning")
        
        assert isinstance(result.skill, str)
        assert isinstance(result.score, float)
        assert isinstance(result.confidence, str)
        assert result.confidence in ["None", "Low", "Medium", "High", "Definite"]


class TestEnforcementPipeline:
    """Tests for EnforcementPipeline."""

    def test_create_pipeline(self):
        """Test creating an EnforcementPipeline."""
        from skm import EnforcementPipeline

        pipeline = EnforcementPipeline()
        assert pipeline is not None

    def test_check_before_allows(self):
        """Test that default pipeline allows activation."""
        from skm import EnforcementPipeline

        pipeline = EnforcementPipeline()
        decision = pipeline.check_before("test-skill", "run test")
        
        assert decision.is_allowed()
        assert not decision.is_cancelled()
        assert decision.decision_type == "allow"

    def test_check_after_allows(self):
        """Test that default pipeline allows execution output."""
        from skm import EnforcementPipeline

        pipeline = EnforcementPipeline()
        decision = pipeline.check_after("test-skill", "Test output")
        
        assert decision.is_allowed()

    def test_check_with_user_context(self):
        """Test enforcement with user context."""
        from skm import EnforcementPipeline

        pipeline = EnforcementPipeline()
        decision = pipeline.check_before(
            "test-skill", 
            "run test",
            user_id="user123",
            session_id="session456"
        )
        
        assert decision.is_allowed()


class TestSkillMetadata:
    """Tests for SkillMetadata attributes."""

    def test_metadata_properties(self):
        """Test all metadata properties are accessible."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        meta = registry.get("test-skill")
        
        assert meta is not None
        
        # Check all properties
        assert meta.name == "test-skill"
        assert len(meta.description) > 0
        assert isinstance(meta.triggers, list)
        assert isinstance(meta.tags, list)
        assert len(meta.source_path) > 0
        assert meta.content_hash > 0
        assert meta.estimated_tokens > 0

    def test_metadata_repr(self):
        """Test metadata repr."""
        from skm import SkillRegistry

        registry = SkillRegistry([FIXTURES_DIR])
        meta = registry.get("test-skill")
        
        repr_str = repr(meta)
        assert "SkillMetadata" in repr_str
        assert "test-skill" in repr_str


# Embedding tests (optional, require model download)
class TestEmbedding:
    """Tests for embedding providers (skipped if not available)."""

    @pytest.mark.skip(reason="Requires model download")
    def test_bge_m3_embed(self):
        """Test BGE-M3 embedding."""
        from skm import BgeM3Provider
        
        if BgeM3Provider is None:
            pytest.skip("BgeM3Provider not available")
        
        provider = BgeM3Provider()
        embedding = provider.embed("Hello, world!")
        
        assert len(embedding) == 1024
        assert provider.dimensions == 1024

    @pytest.mark.skip(reason="Requires model download")
    def test_minilm_embed(self):
        """Test MiniLM embedding."""
        from skm import MiniLmProvider
        
        if MiniLmProvider is None:
            pytest.skip("MiniLmProvider not available")
        
        provider = MiniLmProvider()
        embedding = provider.embed("Hello, world!")
        
        assert len(embedding) == 384
        assert provider.dimensions == 384


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
