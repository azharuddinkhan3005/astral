use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::state::AgentState;
use super::task::{AgentTask, TaskStatus};

/// Summary statistics for an orchestrator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub pending: usize,
    pub running: usize,
}

/// The orchestrator manages a directed acyclic task graph.
///
/// It tracks task dependencies, resolves which tasks are ready to run,
/// and records completion status for each task.
pub struct Orchestrator {
    tasks: Vec<AgentTask>,
    state: Arc<Mutex<AgentState>>,
    pub timeout_seconds: u64,
    pub max_retries: u8,
}

impl Orchestrator {
    pub fn new(timeout_seconds: u64, max_retries: u8) -> Self {
        Self {
            tasks: Vec::new(),
            state: Arc::new(Mutex::new(AgentState::new())),
            timeout_seconds,
            max_retries,
        }
    }

    /// Add a task to the graph.
    pub fn add_task(&mut self, task: AgentTask) {
        self.tasks.push(task);
    }

    /// Return IDs of tasks that are Pending and whose dependencies have all Succeeded.
    pub fn resolve_ready(&self) -> Vec<String> {
        let succeeded: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Succeeded)
            .map(|t| t.id.clone())
            .collect();

        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending && t.is_ready(&succeeded))
            .map(|t| t.id.clone())
            .collect()
    }

    /// Mark a task as Running.
    pub fn mark_running(&mut self, task_id: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.mark_running();
        }
    }

    /// Mark a task as Succeeded with the given output.
    pub fn mark_succeeded(&mut self, task_id: &str, output: serde_json::Value) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.mark_succeeded(output);
        }
    }

    /// Mark a task as Failed with the given error message.
    pub fn mark_failed(&mut self, task_id: &str, error: String) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.mark_failed(error);
        }
    }

    /// Returns true when every task is in a terminal state (Succeeded, Failed, or Skipped).
    pub fn is_complete(&self) -> bool {
        self.tasks.iter().all(|t| {
            matches!(
                t.status,
                TaskStatus::Succeeded | TaskStatus::Failed | TaskStatus::Skipped
            )
        })
    }

    /// Returns true if any task has Failed.
    pub fn has_failures(&self) -> bool {
        self.tasks.iter().any(|t| t.status == TaskStatus::Failed)
    }

    /// Produce a summary of the current orchestrator state.
    pub fn summary(&self) -> OrchestratorSummary {
        let mut s = OrchestratorSummary {
            total: self.tasks.len(),
            succeeded: 0,
            failed: 0,
            skipped: 0,
            pending: 0,
            running: 0,
        };
        for task in &self.tasks {
            match task.status {
                TaskStatus::Succeeded => s.succeeded += 1,
                TaskStatus::Failed => s.failed += 1,
                TaskStatus::Skipped => s.skipped += 1,
                TaskStatus::Running => s.running += 1,
                TaskStatus::Pending | TaskStatus::Ready => s.pending += 1,
            }
        }
        s
    }

    /// Return a clone of the shared state handle.
    pub fn state(&self) -> Arc<Mutex<AgentState>> {
        Arc::clone(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::task::{AgentTask, AgentType, TaskStatus};

    fn task(id: &str, deps: Vec<&str>) -> AgentTask {
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
    fn test_resolve_ready_no_deps() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["walk".to_string()]);
    }

    #[test]
    fn test_resolve_ready_respects_deps() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        orch.add_task(task("parse", vec!["walk"]));
        orch.add_task(task("build", vec!["parse"]));

        // Initially only walk is ready
        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["walk".to_string()]);

        // After walk succeeds, parse becomes ready
        orch.mark_running("walk");
        orch.mark_succeeded("walk", serde_json::json!(null));
        let ready = orch.resolve_ready();
        assert_eq!(ready, vec!["parse".to_string()]);

        // build still not ready
        assert!(!ready.contains(&"build".to_string()));
    }

    #[test]
    fn test_resolve_ready_parallel_tasks() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        orch.add_task(task("parse", vec!["walk"]));
        orch.add_task(task("audit", vec!["parse"]));
        orch.add_task(task("batch", vec!["parse"]));
        orch.add_task(task("docs", vec!["parse"]));

        orch.mark_running("walk");
        orch.mark_succeeded("walk", serde_json::json!(null));
        orch.mark_running("parse");
        orch.mark_succeeded("parse", serde_json::json!(null));

        let mut ready = orch.resolve_ready();
        ready.sort();
        assert_eq!(
            ready,
            vec!["audit".to_string(), "batch".to_string(), "docs".to_string(),]
        );
    }

    #[test]
    fn test_is_complete() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        orch.add_task(task("parse", vec!["walk"]));

        assert!(!orch.is_complete());

        orch.mark_running("walk");
        orch.mark_succeeded("walk", serde_json::json!(null));
        assert!(!orch.is_complete());

        orch.mark_running("parse");
        orch.mark_succeeded("parse", serde_json::json!(null));
        assert!(orch.is_complete());
    }

    #[test]
    fn test_is_complete_with_failure() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        orch.add_task(task("parse", vec!["walk"]));

        orch.mark_running("walk");
        orch.mark_failed("walk", "disk error".to_string());

        // parse is still Pending so not complete
        assert!(!orch.is_complete());
    }

    #[test]
    fn test_has_failures() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));

        assert!(!orch.has_failures());

        orch.mark_running("walk");
        orch.mark_failed("walk", "error".to_string());
        assert!(orch.has_failures());
    }

    #[test]
    fn test_summary() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("a", vec![]));
        orch.add_task(task("b", vec!["a"]));
        orch.add_task(task("c", vec!["a"]));

        orch.mark_running("a");
        orch.mark_succeeded("a", serde_json::json!(null));
        orch.mark_running("b");
        orch.mark_failed("b", "boom".to_string());

        let s = orch.summary();
        assert_eq!(s.total, 3);
        assert_eq!(s.succeeded, 1);
        assert_eq!(s.failed, 1);
        assert_eq!(s.pending, 1);
        assert_eq!(s.running, 0);
        assert_eq!(s.skipped, 0);
    }

    #[test]
    fn test_state_is_shared() {
        let orch = Orchestrator::new(300, 3);
        let s1 = orch.state();
        let s2 = orch.state();
        // Both should point to the same underlying allocation
        assert!(Arc::ptr_eq(&s1, &s2));
    }

    #[test]
    fn test_running_tasks_not_resolved_as_ready() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        orch.mark_running("walk");

        let ready = orch.resolve_ready();
        assert!(ready.is_empty());
    }

    #[test]
    fn test_failed_deps_block_downstream() {
        let mut orch = Orchestrator::new(300, 3);
        orch.add_task(task("walk", vec![]));
        orch.add_task(task("parse", vec!["walk"]));

        orch.mark_running("walk");
        orch.mark_failed("walk", "error".to_string());

        // parse depends on walk, which failed (not Succeeded), so parse is NOT ready
        let ready = orch.resolve_ready();
        assert!(ready.is_empty());
    }
}
