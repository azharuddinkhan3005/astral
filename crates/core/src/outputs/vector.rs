use crate::{AnalysisResult, ResultStatus};
use anyhow::Result;
use serde::Serialize;

/// Render analysis results as vector-ready JSON for embedding stores
/// such as Qdrant, Chroma, Weaviate, or Pinecone.
///
/// Each analysis result becomes a document with its text and structured
/// metadata that can be used for filtering and faceted search.
pub fn render(results: &[AnalysisResult]) -> Result<String> {
    let documents: Vec<VectorDocument> = results.iter().map(VectorDocument::from).collect();
    let envelope = VectorEnvelope { documents };
    Ok(serde_json::to_string_pretty(&envelope)?)
}

// ---------------------------------------------------------------------------
// Serde structs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct VectorEnvelope {
    documents: Vec<VectorDocument>,
}

#[derive(Serialize)]
struct VectorDocument {
    id: String,
    text: String,
    metadata: VectorMetadata,
}

#[derive(Serialize)]
struct VectorMetadata {
    file_path: String,
    chunk_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    status: String,
}

impl From<&AnalysisResult> for VectorDocument {
    fn from(result: &AnalysisResult) -> Self {
        Self {
            id: result.chunk_id.clone(),
            text: result.analysis.clone(),
            metadata: VectorMetadata {
                file_path: result.file_path.clone(),
                chunk_type: format!("{:?}", result.chunk_type).to_lowercase(),
                name: result.name.clone(),
                status: status_str(&result.status),
            },
        }
    }
}

fn status_str(s: &ResultStatus) -> String {
    match s {
        ResultStatus::Succeeded => "succeeded".to_string(),
        ResultStatus::Errored => "errored".to_string(),
        ResultStatus::Canceled => "canceled".to_string(),
        ResultStatus::Expired => "expired".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChunkType;

    fn sample_results() -> Vec<AnalysisResult> {
        vec![
            AnalysisResult {
                chunk_id: "abc123".to_string(),
                file_path: "src/main.ts".to_string(),
                chunk_type: ChunkType::Function,
                name: Some("greet".to_string()),
                analysis: "Greets the user.".to_string(),
                status: ResultStatus::Succeeded,
            },
            AnalysisResult {
                chunk_id: "def456".to_string(),
                file_path: "src/lib.ts".to_string(),
                chunk_type: ChunkType::Class,
                name: None,
                analysis: "Handles user operations.".to_string(),
                status: ResultStatus::Errored,
            },
        ]
    }

    #[test]
    fn test_vector_valid_json() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["documents"].is_array());
    }

    #[test]
    fn test_vector_document_count() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["documents"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_vector_document_fields() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let first = &parsed["documents"][0];

        assert_eq!(first["id"], "abc123");
        assert_eq!(first["text"], "Greets the user.");
        assert_eq!(first["metadata"]["file_path"], "src/main.ts");
        assert_eq!(first["metadata"]["chunk_type"], "function");
        assert_eq!(first["metadata"]["name"], "greet");
        assert_eq!(first["metadata"]["status"], "succeeded");
    }

    #[test]
    fn test_vector_null_name_omitted() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let second = &parsed["documents"][1];

        // name should be absent when None
        assert!(second["metadata"]["name"].is_null());
    }

    #[test]
    fn test_vector_errored_status() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let second = &parsed["documents"][1];

        assert_eq!(second["metadata"]["status"], "errored");
        assert_eq!(second["metadata"]["chunk_type"], "class");
    }

    #[test]
    fn test_vector_empty_results() {
        let output = render(&[]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["documents"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_vector_id_is_chunk_id() {
        let results = vec![AnalysisResult {
            chunk_id: "unique-hash-value".to_string(),
            file_path: "x.py".to_string(),
            chunk_type: ChunkType::Module,
            name: Some("mod".to_string()),
            analysis: "A module.".to_string(),
            status: ResultStatus::Succeeded,
        }];

        let output = render(&results).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["documents"][0]["id"], "unique-hash-value");
    }
}
