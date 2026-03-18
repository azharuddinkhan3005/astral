use crate::aggregator::compute_stats;
use crate::{AnalysisResult, Config, ResultStatus};
use anyhow::Result;
use std::collections::BTreeMap;

/// Render analysis results as a Markdown report.
pub fn render(results: &[AnalysisResult], _config: &Config) -> Result<String> {
    let stats = compute_stats(results);
    let mut out = String::new();

    // Header
    out.push_str("# Astral Analysis Report\n\n");

    // Summary table
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("|--------|-------|\n");
    out.push_str(&format!("| Total chunks | {} |\n", stats.total));
    out.push_str(&format!("| Succeeded | {} |\n", stats.succeeded));
    out.push_str(&format!("| Errored | {} |\n", stats.errored));
    out.push_str(&format!("| Canceled | {} |\n", stats.canceled));
    out.push_str(&format!("| Expired | {} |\n", stats.expired));
    out.push('\n');

    // Group results by file
    let mut by_file: BTreeMap<&str, Vec<&AnalysisResult>> = BTreeMap::new();
    for result in results {
        by_file.entry(&result.file_path).or_default().push(result);
    }

    // File sections
    out.push_str("## Results by File\n\n");

    for (file, file_results) in &by_file {
        out.push_str(&format!("### `{}`\n\n", file));

        for result in file_results {
            let name = result.name.as_deref().unwrap_or("(unnamed)");

            let status_badge = match result.status {
                ResultStatus::Succeeded => "",
                ResultStatus::Errored => " [ERROR]",
                ResultStatus::Canceled => " [CANCELED]",
                ResultStatus::Expired => " [EXPIRED]",
            };

            out.push_str(&format!("#### `{}`{}\n\n", name, status_badge));
            out.push_str(&format!("**Type:** {:?}\n\n", result.chunk_type));
            out.push_str(&result.analysis);
            out.push_str("\n\n---\n\n");
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, Config};

    #[test]
    fn test_markdown_render() {
        let results = vec![AnalysisResult {
            chunk_id: "a".to_string(),
            file_path: "src/main.ts".to_string(),
            chunk_type: ChunkType::Function,
            name: Some("greet".to_string()),
            analysis: "This function greets the user.".to_string(),
            status: ResultStatus::Succeeded,
        }];

        let config = Config::default();
        let output = render(&results, &config).unwrap();

        assert!(output.contains("# Astral Analysis Report"));
        assert!(output.contains("src/main.ts"));
        assert!(output.contains("greet"));
        assert!(output.contains("This function greets the user."));
        assert!(output.contains("| Succeeded | 1 |"));
    }
}
