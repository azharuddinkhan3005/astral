use crate::AnalysisResult;
use anyhow::Result;

/// Render analysis results as JSONL (one JSON object per line).
///
/// JSONL is streaming-friendly — each line is a self-contained JSON object
/// that can be processed independently by tools like `jq`, log aggregators,
/// or incremental consumers.
pub fn render(results: &[AnalysisResult]) -> Result<String> {
    let mut out = String::new();
    for result in results {
        let line = serde_json::to_string(result)?;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, ResultStatus};

    fn sample_results() -> Vec<AnalysisResult> {
        vec![
            AnalysisResult {
                chunk_id: "a1".to_string(),
                file_path: "src/main.ts".to_string(),
                chunk_type: ChunkType::Function,
                name: Some("greet".to_string()),
                analysis: "Greets the user.".to_string(),
                status: ResultStatus::Succeeded,
            },
            AnalysisResult {
                chunk_id: "b2".to_string(),
                file_path: "src/lib.ts".to_string(),
                chunk_type: ChunkType::Class,
                name: Some("UserService".to_string()),
                analysis: "Handles user operations.".to_string(),
                status: ResultStatus::Errored,
            },
        ]
    }

    #[test]
    fn test_jsonl_one_line_per_result() {
        let output = render(&sample_results()).unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_jsonl_each_line_is_valid_json() {
        let output = render(&sample_results()).unwrap();
        for line in output.lines() {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.is_object());
        }
    }

    #[test]
    fn test_jsonl_fields_present() {
        let output = render(&sample_results()).unwrap();
        let first_line = output.lines().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(first_line).unwrap();

        assert_eq!(parsed["chunk_id"], "a1");
        assert_eq!(parsed["file_path"], "src/main.ts");
        assert_eq!(parsed["chunk_type"], "function");
        assert_eq!(parsed["name"], "greet");
        assert_eq!(parsed["analysis"], "Greets the user.");
        assert_eq!(parsed["status"], "succeeded");
    }

    #[test]
    fn test_jsonl_no_pretty_printing() {
        let output = render(&sample_results()).unwrap();
        for line in output.lines() {
            // Each line must be a single line (no embedded newlines)
            assert!(!line.contains('\n'));
            assert!(!line.contains("  "), "line should not be pretty-printed");
        }
    }

    #[test]
    fn test_jsonl_empty_results() {
        let output = render(&[]).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn test_jsonl_trailing_newline() {
        let results = vec![AnalysisResult {
            chunk_id: "x".to_string(),
            file_path: "a.rs".to_string(),
            chunk_type: ChunkType::Module,
            name: None,
            analysis: "ok".to_string(),
            status: ResultStatus::Succeeded,
        }];

        let output = render(&results).unwrap();
        assert!(output.ends_with('\n'));
    }
}
