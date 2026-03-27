use crate::AnalysisResult;
use crate::aggregator::compute_stats;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
struct JsonReport {
    stats: crate::aggregator::AggregationStats,
    results: Vec<AnalysisResult>,
}

/// Render analysis results as a JSON report.
pub fn render(results: &[AnalysisResult]) -> Result<String> {
    let report = JsonReport {
        stats: compute_stats(results),
        results: results.to_vec(),
    };

    Ok(serde_json::to_string_pretty(&report)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, ResultStatus};

    #[test]
    fn test_json_render() {
        let results = vec![AnalysisResult {
            chunk_id: "a".to_string(),
            file_path: "src/main.ts".to_string(),
            chunk_type: ChunkType::Function,
            name: Some("greet".to_string()),
            analysis: "Greets the user.".to_string(),
            status: ResultStatus::Succeeded,
        }];

        let output = render(&results).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["stats"]["total"], 1);
        assert_eq!(parsed["stats"]["succeeded"], 1);
        assert_eq!(parsed["results"][0]["chunk_id"], "a");
    }
}
