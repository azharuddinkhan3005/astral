"""Built-in agent pipeline definitions."""

from __future__ import annotations

from py.agent.runtime import PipelineStep


def full_review_pipeline() -> list[PipelineStep]:
    """Complete code review: summarise -> review -> security audit."""
    return [
        PipelineStep(
            name="summarise",
            analysis_mode="summarise",
        ),
        PipelineStep(
            name="code_review",
            analysis_mode="code_review",
        ),
        PipelineStep(
            name="security_audit",
            analysis_mode="security_audit",
        ),
    ]


def doc_generation_pipeline() -> list[PipelineStep]:
    """Generate documentation for the entire codebase."""
    return [
        PipelineStep(
            name="doc_generation",
            analysis_mode="doc_generation",
        ),
    ]


def security_focused_pipeline() -> list[PipelineStep]:
    """Security-first pipeline: audit then review flagged areas."""
    return [
        PipelineStep(
            name="security_audit",
            analysis_mode="security_audit",
        ),
        PipelineStep(
            name="code_review",
            analysis_mode="code_review",
            depends_on=["security_audit"],
        ),
    ]


def test_generation_pipeline() -> list[PipelineStep]:
    """Generate tests: summarise first, then produce tests."""
    return [
        PipelineStep(
            name="summarise",
            analysis_mode="summarise",
        ),
        PipelineStep(
            name="test_generation",
            analysis_mode="test_generation",
            depends_on=["summarise"],
        ),
    ]


BUILTIN_PIPELINES: dict[str, list[PipelineStep]] = {
    "full-review": full_review_pipeline(),
    "doc-gen": doc_generation_pipeline(),
    "security": security_focused_pipeline(),
    "test-gen": test_generation_pipeline(),
}
