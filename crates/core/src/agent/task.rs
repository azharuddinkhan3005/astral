use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single task in the agent task graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub agent: AgentType,
    pub input: serde_json::Value,
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

/// Current status of a task in the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

/// The type of agent that will execute a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentType {
    Orchestrator,
    Walker,
    Parser(String),
    BatchBuilder,
    BatchMonitor,
    ResultAggregator,
    OutputRenderer,
    TestRunner,
    SecurityAudit,
    DocGenerator,
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl AgentTask {
    /// Returns true if all dependency task IDs appear in the completed list.
    pub fn is_ready(&self, completed: &[String]) -> bool {
        self.dependencies.iter().all(|dep| completed.contains(dep))
    }

    /// Transition to Running status and record the start timestamp.
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(now_epoch_secs());
    }

    /// Transition to Succeeded status with the given output.
    pub fn mark_succeeded(&mut self, output: serde_json::Value) {
        self.status = TaskStatus::Succeeded;
        self.output = Some(output);
        self.completed_at = Some(now_epoch_secs());
    }

    /// Transition to Failed status with the given error message.
    pub fn mark_failed(&mut self, error: String) {
        self.status = TaskStatus::Failed;
        self.error = Some(error);
        self.completed_at = Some(now_epoch_secs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task(id: &str, deps: Vec<&str>) -> AgentTask {
        AgentTask {
            id: id.to_string(),
            agent: AgentType::Walker,
            input: serde_json::json!({}),
            dependencies: deps.into_iter().map(String::from).collect(),
            status: TaskStatus::Pending,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn test_is_ready_no_deps() {
        let task = sample_task("t1", vec![]);
        assert!(task.is_ready(&[]));
    }

    #[test]
    fn test_is_ready_deps_satisfied() {
        let task = sample_task("t2", vec!["t1"]);
        assert!(task.is_ready(&["t1".to_string()]));
    }

    #[test]
    fn test_is_ready_deps_not_satisfied() {
        let task = sample_task("t2", vec!["t1", "t0"]);
        assert!(!task.is_ready(&["t1".to_string()]));
    }

    #[test]
    fn test_mark_running() {
        let mut task = sample_task("t1", vec![]);
        task.mark_running();
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
    }

    #[test]
    fn test_mark_succeeded() {
        let mut task = sample_task("t1", vec![]);
        task.mark_running();
        task.mark_succeeded(serde_json::json!({"count": 42}));
        assert_eq!(task.status, TaskStatus::Succeeded);
        assert!(task.output.is_some());
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_mark_failed() {
        let mut task = sample_task("t1", vec![]);
        task.mark_running();
        task.mark_failed("timeout".to_string());
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.error.as_deref(), Some("timeout"));
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_transitions_preserve_started_at() {
        let mut task = sample_task("t1", vec![]);
        task.mark_running();
        let started = task.started_at;
        task.mark_succeeded(serde_json::json!(null));
        // started_at should not be overwritten by succeeded
        assert_eq!(task.started_at, started);
    }
}
