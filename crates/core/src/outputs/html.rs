use crate::aggregator::compute_stats;
use crate::{AnalysisResult, Config, ResultStatus};
use anyhow::Result;
use std::collections::BTreeMap;

/// Render analysis results as an HTML report.
pub fn render(results: &[AnalysisResult], _config: &Config) -> Result<String> {
    let stats = compute_stats(results);

    let mut by_file: BTreeMap<&str, Vec<&AnalysisResult>> = BTreeMap::new();
    for result in results {
        by_file.entry(&result.file_path).or_default().push(result);
    }

    let mut html = String::new();

    html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Astral Analysis Report</title>
<style>
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 960px; margin: 0 auto; padding: 2rem; color: #333; }
  h1 { border-bottom: 2px solid #6366f1; padding-bottom: 0.5rem; }
  h2 { color: #4f46e5; }
  h3 { color: #6366f1; }
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 1rem; margin: 1.5rem 0; }
  .stat { background: #f5f3ff; border-radius: 8px; padding: 1rem; text-align: center; }
  .stat-value { font-size: 2rem; font-weight: bold; color: #4f46e5; }
  .stat-label { font-size: 0.85rem; color: #6b7280; }
  .result { background: #fafafa; border: 1px solid #e5e7eb; border-radius: 8px; padding: 1.5rem; margin: 1rem 0; }
  .result-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
  .badge { font-size: 0.75rem; padding: 2px 8px; border-radius: 12px; font-weight: 600; }
  .badge-success { background: #dcfce7; color: #166534; }
  .badge-error { background: #fee2e2; color: #991b1b; }
  .badge-warn { background: #fef3c7; color: #92400e; }
  .analysis { white-space: pre-wrap; line-height: 1.6; }
  code { background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-size: 0.9em; }
</style>
</head>
<body>
<h1>Astral Analysis Report</h1>
"#);

    // Stats
    html.push_str("<div class=\"stats\">\n");
    html.push_str(&format!(
        "<div class=\"stat\"><div class=\"stat-value\">{}</div><div class=\"stat-label\">Total</div></div>\n",
        stats.total
    ));
    html.push_str(&format!(
        "<div class=\"stat\"><div class=\"stat-value\">{}</div><div class=\"stat-label\">Succeeded</div></div>\n",
        stats.succeeded
    ));
    html.push_str(&format!(
        "<div class=\"stat\"><div class=\"stat-value\">{}</div><div class=\"stat-label\">Errored</div></div>\n",
        stats.errored
    ));
    html.push_str("</div>\n\n");

    // Results by file
    for (file, file_results) in &by_file {
        html.push_str(&format!("<h2><code>{}</code></h2>\n", html_escape(file)));

        for result in file_results {
            let name = result.name.as_deref().unwrap_or("(unnamed)");
            let badge = match result.status {
                ResultStatus::Succeeded => "<span class=\"badge badge-success\">OK</span>",
                ResultStatus::Errored => "<span class=\"badge badge-error\">ERROR</span>",
                ResultStatus::Canceled => "<span class=\"badge badge-warn\">CANCELED</span>",
                ResultStatus::Expired => "<span class=\"badge badge-warn\">EXPIRED</span>",
            };

            html.push_str("<div class=\"result\">\n");
            html.push_str(&format!(
                "<div class=\"result-header\"><h3><code>{}</code></h3>{}</div>\n",
                html_escape(name),
                badge
            ));
            html.push_str(&format!(
                "<div class=\"analysis\">{}</div>\n",
                html_escape(&result.analysis)
            ));
            html.push_str("</div>\n");
        }
    }

    html.push_str("</body>\n</html>\n");

    Ok(html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, Config};

    #[test]
    fn test_html_render() {
        let results = vec![AnalysisResult {
            chunk_id: "a".to_string(),
            file_path: "src/main.ts".to_string(),
            chunk_type: ChunkType::Function,
            name: Some("greet".to_string()),
            analysis: "Greets the user.".to_string(),
            status: ResultStatus::Succeeded,
        }];

        let config = Config::default();
        let output = render(&results, &config).unwrap();

        assert!(output.contains("<!DOCTYPE html>"));
        assert!(output.contains("Astral Analysis Report"));
        assert!(output.contains("greet"));
        assert!(output.contains("badge-success"));
    }
}
