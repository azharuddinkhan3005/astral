//! Agent runtime for multi-step analysis pipelines.
//!
//! The agent module provides the infrastructure for building multi-step
//! analysis workflows. Agents can chain analysis results, branch based
//! on findings, and compose complex codebase investigations.

pub mod orchestrator;
pub mod pipeline;
pub mod state;
pub mod task;

pub use orchestrator::{Orchestrator, OrchestratorSummary};
pub use pipeline::{
    build_analyse_pipeline, build_full_pipeline, build_review_pipeline, build_test_pipeline,
};
pub use state::AgentState;
pub use task::{AgentTask, AgentType, TaskStatus};

use crate::{AnalysisResult, CodeChunk, Config};
use serde::{Deserialize, Serialize};

/// A single step in an agent pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub id: String,
    pub name: String,
    pub description: String,
    pub analysis_mode: crate::AnalysisMode,
    /// Optional filter: only process chunks matching these file patterns.
    #[serde(default)]
    pub file_filter: Vec<String>,
    /// Optional: only process chunks where a previous step flagged issues.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// An agent pipeline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPipeline {
    pub name: String,
    pub description: String,
    pub steps: Vec<AgentStep>,
}

/// Result of running an agent pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub pipeline_name: String,
    pub step_results: Vec<StepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub step_name: String,
    pub results: Vec<AnalysisResult>,
    pub chunks_processed: usize,
}

/// Built-in pipeline: full code review (scan → review → security audit).
pub fn full_review_pipeline() -> AgentPipeline {
    AgentPipeline {
        name: "full-review".to_string(),
        description: "Complete code review: summarise → review → security audit".to_string(),
        steps: vec![
            AgentStep {
                id: "summarise".to_string(),
                name: "Summarise".to_string(),
                description: "Generate summaries for all code chunks".to_string(),
                analysis_mode: crate::AnalysisMode::Summarise,
                file_filter: vec![],
                depends_on: vec![],
            },
            AgentStep {
                id: "review".to_string(),
                name: "Code Review".to_string(),
                description: "Perform code review on all chunks".to_string(),
                analysis_mode: crate::AnalysisMode::CodeReview,
                file_filter: vec![],
                depends_on: vec![],
            },
            AgentStep {
                id: "security".to_string(),
                name: "Security Audit".to_string(),
                description: "Run security audit on all chunks".to_string(),
                analysis_mode: crate::AnalysisMode::SecurityAudit,
                file_filter: vec![],
                depends_on: vec![],
            },
        ],
    }
}

/// Built-in pipeline: documentation generation.
pub fn doc_pipeline() -> AgentPipeline {
    AgentPipeline {
        name: "doc-gen".to_string(),
        description: "Generate documentation for all code".to_string(),
        steps: vec![AgentStep {
            id: "docs".to_string(),
            name: "Documentation".to_string(),
            description: "Generate inline documentation".to_string(),
            analysis_mode: crate::AnalysisMode::DocGeneration,
            file_filter: vec![],
            depends_on: vec![],
        }],
    }
}

/// Filter chunks based on agent step criteria.
pub fn filter_chunks_for_step<'a>(
    chunks: &'a [CodeChunk],
    step: &AgentStep,
    _previous_results: &[StepResult],
) -> Vec<&'a CodeChunk> {
    let mut filtered: Vec<&CodeChunk> = chunks.iter().collect();

    // Apply file filters
    if !step.file_filter.is_empty() {
        filtered.retain(|chunk| {
            step.file_filter.iter().any(|pattern| {
                glob::Pattern::new(pattern)
                    .map(|g| g.matches(&chunk.file_path))
                    .unwrap_or(false)
            })
        });
    }

    filtered
}

/// Build a Config for a specific agent step.
pub fn config_for_step(base_config: &Config, step: &AgentStep) -> Config {
    let mut config = base_config.clone();
    config.analysis_mode = step.analysis_mode.clone();
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_review_pipeline_structure() {
        let pipeline = full_review_pipeline();
        assert_eq!(pipeline.steps.len(), 3);
        assert_eq!(pipeline.steps[0].id, "summarise");
        assert_eq!(pipeline.steps[1].id, "review");
        assert_eq!(pipeline.steps[2].id, "security");
    }

    #[test]
    fn test_doc_pipeline_structure() {
        let pipeline = doc_pipeline();
        assert_eq!(pipeline.steps.len(), 1);
        assert_eq!(pipeline.steps[0].id, "docs");
    }

    #[test]
    fn test_filter_chunks_no_filter() {
        let chunks = vec![crate::CodeChunk {
            id: "a".to_string(),
            file_path: "src/main.ts".to_string(),
            language: "typescript".to_string(),
            chunk_type: crate::ChunkType::Function,
            name: Some("test".to_string()),
            content: "fn test() {}".to_string(),
            start_line: 1,
            end_line: 1,
            imports: vec![],
        }];

        let step = AgentStep {
            id: "s1".to_string(),
            name: "Test".to_string(),
            description: "Test step".to_string(),
            analysis_mode: crate::AnalysisMode::Summarise,
            file_filter: vec![],
            depends_on: vec![],
        };

        let filtered = filter_chunks_for_step(&chunks, &step, &[]);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_filter_chunks_with_glob() {
        let chunks = vec![
            crate::CodeChunk {
                id: "a".to_string(),
                file_path: "src/main.ts".to_string(),
                language: "typescript".to_string(),
                chunk_type: crate::ChunkType::Function,
                name: None,
                content: String::new(),
                start_line: 1,
                end_line: 1,
                imports: vec![],
            },
            crate::CodeChunk {
                id: "b".to_string(),
                file_path: "src/utils.py".to_string(),
                language: "python".to_string(),
                chunk_type: crate::ChunkType::Function,
                name: None,
                content: String::new(),
                start_line: 1,
                end_line: 1,
                imports: vec![],
            },
        ];

        let step = AgentStep {
            id: "s1".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            analysis_mode: crate::AnalysisMode::Summarise,
            file_filter: vec!["**/*.ts".to_string()],
            depends_on: vec![],
        };

        let filtered = filter_chunks_for_step(&chunks, &step, &[]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].file_path, "src/main.ts");
    }

    #[test]
    fn test_config_for_step() {
        let base = Config::default();
        let step = AgentStep {
            id: "s1".to_string(),
            name: "Security".to_string(),
            description: "Security audit".to_string(),
            analysis_mode: crate::AnalysisMode::SecurityAudit,
            file_filter: vec![],
            depends_on: vec![],
        };

        let config = config_for_step(&base, &step);
        assert_eq!(config.analysis_mode, crate::AnalysisMode::SecurityAudit);
        // Other fields should be inherited
        assert_eq!(config.model, base.model);
    }
}
