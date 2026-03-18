use crate::Config;

use super::task::{AgentTask, AgentType, TaskStatus};

/// Build the standard analysis pipeline:
///   walk -> parse -> batch_build -> render
pub fn build_analyse_pipeline(repo_path: &str, config: &Config) -> Vec<AgentTask> {
    let input = serde_json::json!({
        "repo_path": repo_path,
        "model": config.model,
        "output_dir": config.output_dir,
    });

    vec![
        AgentTask {
            id: "walk".to_string(),
            agent: AgentType::Walker,
            input: input.clone(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "parse".to_string(),
            agent: AgentType::Parser("auto".to_string()),
            input: input.clone(),
            dependencies: vec!["walk".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "batch_build".to_string(),
            agent: AgentType::BatchBuilder,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "render".to_string(),
            agent: AgentType::OutputRenderer,
            input,
            dependencies: vec!["batch_build".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
    ]
}

/// Build the code review pipeline:
///   walk -> parse -> [security_audit, code_review] -> render_sarif
pub fn build_review_pipeline(repo_path: &str, config: &Config) -> Vec<AgentTask> {
    let input = serde_json::json!({
        "repo_path": repo_path,
        "model": config.model,
        "output_dir": config.output_dir,
    });

    vec![
        AgentTask {
            id: "walk".to_string(),
            agent: AgentType::Walker,
            input: input.clone(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "parse".to_string(),
            agent: AgentType::Parser("auto".to_string()),
            input: input.clone(),
            dependencies: vec!["walk".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "security_audit".to_string(),
            agent: AgentType::SecurityAudit,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "code_review".to_string(),
            agent: AgentType::BatchBuilder,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "render_sarif".to_string(),
            agent: AgentType::OutputRenderer,
            input,
            dependencies: vec!["security_audit".to_string(), "code_review".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
    ]
}

/// Build the test generation pipeline:
///   walk -> parse -> test_runner -> render
pub fn build_test_pipeline(repo_path: &str, config: &Config) -> Vec<AgentTask> {
    let input = serde_json::json!({
        "repo_path": repo_path,
        "model": config.model,
        "output_dir": config.output_dir,
    });

    vec![
        AgentTask {
            id: "walk".to_string(),
            agent: AgentType::Walker,
            input: input.clone(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "parse".to_string(),
            agent: AgentType::Parser("auto".to_string()),
            input: input.clone(),
            dependencies: vec!["walk".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "test_runner".to_string(),
            agent: AgentType::TestRunner,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "render".to_string(),
            agent: AgentType::OutputRenderer,
            input,
            dependencies: vec!["test_runner".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
    ]
}

/// Build the full pipeline with parallel middle stages:
///   walk -> parse -> [batch_build, security_audit, doc_gen] -> render_all
pub fn build_full_pipeline(repo_path: &str, config: &Config) -> Vec<AgentTask> {
    let input = serde_json::json!({
        "repo_path": repo_path,
        "model": config.model,
        "output_dir": config.output_dir,
    });

    vec![
        AgentTask {
            id: "walk".to_string(),
            agent: AgentType::Walker,
            input: input.clone(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "parse".to_string(),
            agent: AgentType::Parser("auto".to_string()),
            input: input.clone(),
            dependencies: vec!["walk".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "batch_build".to_string(),
            agent: AgentType::BatchBuilder,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "security_audit".to_string(),
            agent: AgentType::SecurityAudit,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "doc_gen".to_string(),
            agent: AgentType::DocGenerator,
            input: input.clone(),
            dependencies: vec!["parse".to_string()],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
        AgentTask {
            id: "render_all".to_string(),
            agent: AgentType::OutputRenderer,
            input,
            dependencies: vec![
                "batch_build".to_string(),
                "security_audit".to_string(),
                "doc_gen".to_string(),
            ],
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
    }

    // -- analyse pipeline -------------------------------------------------

    #[test]
    fn test_analyse_pipeline_structure() {
        let tasks = build_analyse_pipeline("/repo", &default_config());
        assert_eq!(tasks.len(), 4);

        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["walk", "parse", "batch_build", "render"]);
    }

    #[test]
    fn test_analyse_pipeline_deps() {
        let tasks = build_analyse_pipeline("/repo", &default_config());
        assert!(tasks[0].dependencies.is_empty()); // walk
        assert_eq!(tasks[1].dependencies, vec!["walk"]);
        assert_eq!(tasks[2].dependencies, vec!["parse"]);
        assert_eq!(tasks[3].dependencies, vec!["batch_build"]);
    }

    #[test]
    fn test_analyse_pipeline_all_pending() {
        let tasks = build_analyse_pipeline("/repo", &default_config());
        for t in &tasks {
            assert_eq!(t.status, TaskStatus::Pending);
        }
    }

    // -- review pipeline --------------------------------------------------

    #[test]
    fn test_review_pipeline_structure() {
        let tasks = build_review_pipeline("/repo", &default_config());
        assert_eq!(tasks.len(), 5);

        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "walk",
                "parse",
                "security_audit",
                "code_review",
                "render_sarif"
            ]
        );
    }

    #[test]
    fn test_review_pipeline_parallel_middle() {
        let tasks = build_review_pipeline("/repo", &default_config());
        // Both security_audit and code_review depend only on parse
        let audit = tasks.iter().find(|t| t.id == "security_audit").unwrap();
        let review = tasks.iter().find(|t| t.id == "code_review").unwrap();
        assert_eq!(audit.dependencies, vec!["parse"]);
        assert_eq!(review.dependencies, vec!["parse"]);
    }

    #[test]
    fn test_review_pipeline_render_waits_for_both() {
        let tasks = build_review_pipeline("/repo", &default_config());
        let render = tasks.iter().find(|t| t.id == "render_sarif").unwrap();
        assert!(render.dependencies.contains(&"security_audit".to_string()));
        assert!(render.dependencies.contains(&"code_review".to_string()));
    }

    // -- test pipeline ----------------------------------------------------

    #[test]
    fn test_test_pipeline_structure() {
        let tasks = build_test_pipeline("/repo", &default_config());
        assert_eq!(tasks.len(), 4);

        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["walk", "parse", "test_runner", "render"]);
    }

    #[test]
    fn test_test_pipeline_deps() {
        let tasks = build_test_pipeline("/repo", &default_config());
        assert!(tasks[0].dependencies.is_empty());
        assert_eq!(tasks[1].dependencies, vec!["walk"]);
        assert_eq!(tasks[2].dependencies, vec!["parse"]);
        assert_eq!(tasks[3].dependencies, vec!["test_runner"]);
    }

    // -- full pipeline ----------------------------------------------------

    #[test]
    fn test_full_pipeline_structure() {
        let tasks = build_full_pipeline("/repo", &default_config());
        assert_eq!(tasks.len(), 6);

        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "walk",
                "parse",
                "batch_build",
                "security_audit",
                "doc_gen",
                "render_all"
            ]
        );
    }

    #[test]
    fn test_full_pipeline_parallel_middle() {
        let tasks = build_full_pipeline("/repo", &default_config());
        let batch = tasks.iter().find(|t| t.id == "batch_build").unwrap();
        let audit = tasks.iter().find(|t| t.id == "security_audit").unwrap();
        let docs = tasks.iter().find(|t| t.id == "doc_gen").unwrap();

        assert_eq!(batch.dependencies, vec!["parse"]);
        assert_eq!(audit.dependencies, vec!["parse"]);
        assert_eq!(docs.dependencies, vec!["parse"]);
    }

    #[test]
    fn test_full_pipeline_render_waits_for_all() {
        let tasks = build_full_pipeline("/repo", &default_config());
        let render = tasks.iter().find(|t| t.id == "render_all").unwrap();
        assert_eq!(render.dependencies.len(), 3);
        assert!(render.dependencies.contains(&"batch_build".to_string()));
        assert!(render.dependencies.contains(&"security_audit".to_string()));
        assert!(render.dependencies.contains(&"doc_gen".to_string()));
    }

    // -- integration: pipeline + orchestrator ----------------------------

    #[test]
    fn test_pipeline_flows_through_orchestrator() {
        use crate::agent::orchestrator::Orchestrator;

        let tasks = build_analyse_pipeline("/repo", &default_config());
        let mut orch = Orchestrator::new(300, 3);
        for t in tasks {
            orch.add_task(t);
        }

        // Step through the whole pipeline
        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["walk"]);

        orch.mark_running("walk");
        orch.mark_succeeded("walk", serde_json::json!(null));

        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["parse"]);

        orch.mark_running("parse");
        orch.mark_succeeded("parse", serde_json::json!(null));

        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["batch_build"]);

        orch.mark_running("batch_build");
        orch.mark_succeeded("batch_build", serde_json::json!(null));

        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["render"]);

        orch.mark_running("render");
        orch.mark_succeeded("render", serde_json::json!(null));

        assert!(orch.is_complete());
        assert!(!orch.has_failures());

        let s = orch.summary();
        assert_eq!(s.total, 4);
        assert_eq!(s.succeeded, 4);
    }
}
