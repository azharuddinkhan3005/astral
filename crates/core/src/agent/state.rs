use crate::walker::FileEntry;
use crate::{AnalysisResult, BatchRequest, CodeChunk};

/// Shared mutable state that flows through the agent pipeline.
///
/// Each stage writes its outputs here so downstream stages can read them.
#[derive(Debug, Default)]
pub struct AgentState {
    pub file_list: Option<Vec<FileEntry>>,
    pub chunks: Vec<CodeChunk>,
    pub batch_requests: Vec<BatchRequest>,
    pub batch_id: Option<String>,
    pub analysis_results: Vec<AnalysisResult>,
    pub output_paths: Vec<String>,
}

impl AgentState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_file_list(&mut self, files: Vec<FileEntry>) {
        self.file_list = Some(files);
    }

    pub fn set_chunks(&mut self, chunks: Vec<CodeChunk>) {
        self.chunks = chunks;
    }

    pub fn set_batch_id(&mut self, id: String) {
        self.batch_id = Some(id);
    }

    pub fn set_results(&mut self, results: Vec<AnalysisResult>) {
        self.analysis_results = results;
    }

    pub fn add_output_path(&mut self, path: String) {
        self.output_paths.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_is_empty() {
        let state = AgentState::new();
        assert!(state.file_list.is_none());
        assert!(state.chunks.is_empty());
        assert!(state.batch_requests.is_empty());
        assert!(state.batch_id.is_none());
        assert!(state.analysis_results.is_empty());
        assert!(state.output_paths.is_empty());
    }

    #[test]
    fn test_set_file_list() {
        let mut state = AgentState::new();
        let files = vec![FileEntry {
            path: "src/main.rs".to_string(),
            language: "rust".to_string(),
            size_bytes: 100,
            line_count: 10,
        }];
        state.set_file_list(files);
        assert_eq!(state.file_list.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_set_chunks() {
        let mut state = AgentState::new();
        let chunks = vec![CodeChunk {
            id: "c1".to_string(),
            file_path: "src/main.rs".to_string(),
            language: "rust".to_string(),
            chunk_type: crate::ChunkType::Function,
            name: Some("main".to_string()),
            content: "fn main() {}".to_string(),
            start_line: 1,
            end_line: 1,
            imports: vec![],
        }];
        state.set_chunks(chunks);
        assert_eq!(state.chunks.len(), 1);
    }

    #[test]
    fn test_set_batch_id() {
        let mut state = AgentState::new();
        state.set_batch_id("batch_abc123".to_string());
        assert_eq!(state.batch_id.as_deref(), Some("batch_abc123"));
    }

    #[test]
    fn test_add_output_path() {
        let mut state = AgentState::new();
        state.add_output_path("/tmp/report.md".to_string());
        state.add_output_path("/tmp/report.json".to_string());
        assert_eq!(state.output_paths.len(), 2);
    }
}
