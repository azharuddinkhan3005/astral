use crate::{AnalysisResult, CodeChunk, ResultStatus};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// Raw result from the Batch API JSONL output.
#[derive(Debug, Deserialize)]
pub struct RawBatchResult {
    pub custom_id: String,
    pub result: RawResultBody,
}

#[derive(Debug, Deserialize)]
pub struct RawResultBody {
    #[serde(rename = "type")]
    pub result_type: String,
    pub message: Option<RawMessage>,
    pub error: Option<RawError>,
}

#[derive(Debug, Deserialize)]
pub struct RawMessage {
    pub content: Vec<RawContent>,
}

#[derive(Debug, Deserialize)]
pub struct RawContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Aggregate raw JSONL results back to AnalysisResult structs,
/// mapping each result to its original CodeChunk via custom_id.
pub fn aggregate(raw_jsonl: &str, chunks: &[CodeChunk]) -> Result<Vec<AnalysisResult>> {
    let chunk_map: HashMap<&str, &CodeChunk> = chunks.iter().map(|c| (c.id.as_str(), c)).collect();

    let mut results = Vec::new();

    for line in raw_jsonl.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let raw: RawBatchResult = serde_json::from_str(trimmed).map_err(|e| {
            anyhow::anyhow!("Failed to parse JSONL line: {} — line: {}", e, trimmed)
        })?;

        let chunk = chunk_map.get(raw.custom_id.as_str());

        let (analysis, status) = match raw.result.result_type.as_str() {
            "succeeded" => {
                let text = raw
                    .result
                    .message
                    .and_then(|m| m.content.into_iter().filter_map(|c| c.text).next())
                    .unwrap_or_default();
                (text, ResultStatus::Succeeded)
            }
            "errored" => {
                let msg = raw
                    .result
                    .error
                    .map(|e| format!("{}: {}", e.error_type, e.message))
                    .unwrap_or_else(|| "Unknown error".to_string());
                (msg, ResultStatus::Errored)
            }
            "canceled" => ("Request was canceled".to_string(), ResultStatus::Canceled),
            "expired" => ("Request expired".to_string(), ResultStatus::Expired),
            other => (
                format!("Unknown result type: {}", other),
                ResultStatus::Errored,
            ),
        };

        results.push(AnalysisResult {
            chunk_id: raw.custom_id.clone(),
            file_path: chunk
                .map(|c| c.file_path.clone())
                .unwrap_or_else(|| format!("unknown:{}", raw.custom_id)),
            chunk_type: chunk
                .map(|c| c.chunk_type.clone())
                .unwrap_or(crate::ChunkType::Block),
            name: chunk.and_then(|c| c.name.clone()),
            analysis,
            status,
        });
    }

    Ok(results)
}

/// Compute aggregation statistics.
pub fn compute_stats(results: &[AnalysisResult]) -> AggregationStats {
    let succeeded = results
        .iter()
        .filter(|r| r.status == ResultStatus::Succeeded)
        .count();
    let errored = results
        .iter()
        .filter(|r| r.status == ResultStatus::Errored)
        .count();
    let canceled = results
        .iter()
        .filter(|r| r.status == ResultStatus::Canceled)
        .count();
    let expired = results
        .iter()
        .filter(|r| r.status == ResultStatus::Expired)
        .count();

    AggregationStats {
        total: results.len(),
        succeeded,
        errored,
        canceled,
        expired,
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AggregationStats {
    pub total: usize,
    pub succeeded: usize,
    pub errored: usize,
    pub canceled: usize,
    pub expired: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, CodeChunk};

    fn sample_chunks() -> Vec<CodeChunk> {
        vec![
            CodeChunk {
                id: "chunk_a".to_string(),
                file_path: "src/main.ts".to_string(),
                language: "typescript".to_string(),
                chunk_type: ChunkType::Function,
                name: Some("greet".to_string()),
                content: "function greet() {}".to_string(),
                start_line: 1,
                end_line: 1,
                imports: vec![],
            },
            CodeChunk {
                id: "chunk_b".to_string(),
                file_path: "src/utils.ts".to_string(),
                language: "typescript".to_string(),
                chunk_type: ChunkType::Function,
                name: Some("add".to_string()),
                content: "function add() {}".to_string(),
                start_line: 1,
                end_line: 1,
                imports: vec![],
            },
        ]
    }

    #[test]
    fn test_aggregate_succeeded() {
        let jsonl = r#"{"custom_id":"chunk_a","result":{"type":"succeeded","message":{"content":[{"type":"text","text":"This function greets the user."}]}}}
{"custom_id":"chunk_b","result":{"type":"succeeded","message":{"content":[{"type":"text","text":"This function adds two numbers."}]}}}"#;

        let chunks = sample_chunks();
        let results = aggregate(jsonl, &chunks).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk_id, "chunk_a");
        assert_eq!(results[0].status, ResultStatus::Succeeded);
        assert!(results[0].analysis.contains("greets"));
        assert_eq!(results[0].file_path, "src/main.ts");
        assert_eq!(results[0].name, Some("greet".to_string()));
    }

    #[test]
    fn test_aggregate_errored() {
        let jsonl = r#"{"custom_id":"chunk_a","result":{"type":"errored","error":{"type":"rate_limit","message":"Too many requests"}}}"#;
        let chunks = sample_chunks();
        let results = aggregate(jsonl, &chunks).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, ResultStatus::Errored);
        assert!(results[0].analysis.contains("rate_limit"));
    }

    #[test]
    fn test_aggregate_mixed_statuses() {
        let jsonl = r#"{"custom_id":"chunk_a","result":{"type":"succeeded","message":{"content":[{"type":"text","text":"OK"}]}}}
{"custom_id":"chunk_b","result":{"type":"expired"}}"#;

        let chunks = sample_chunks();
        let results = aggregate(jsonl, &chunks).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, ResultStatus::Succeeded);
        assert_eq!(results[1].status, ResultStatus::Expired);
    }

    #[test]
    fn test_aggregate_unknown_chunk_id() {
        let jsonl = r#"{"custom_id":"unknown_id","result":{"type":"succeeded","message":{"content":[{"type":"text","text":"OK"}]}}}"#;
        let chunks = sample_chunks();
        let results = aggregate(jsonl, &chunks).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("unknown"));
    }

    #[test]
    fn test_compute_stats() {
        let results = vec![
            AnalysisResult {
                chunk_id: "a".to_string(),
                file_path: "a.ts".to_string(),
                chunk_type: ChunkType::Function,
                name: None,
                analysis: "ok".to_string(),
                status: ResultStatus::Succeeded,
            },
            AnalysisResult {
                chunk_id: "b".to_string(),
                file_path: "b.ts".to_string(),
                chunk_type: ChunkType::Function,
                name: None,
                analysis: "err".to_string(),
                status: ResultStatus::Errored,
            },
        ];

        let stats = compute_stats(&results);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.succeeded, 1);
        assert_eq!(stats.errored, 1);
    }

    #[test]
    fn test_empty_jsonl() {
        let results = aggregate("", &[]).unwrap();
        assert!(results.is_empty());
    }
}
