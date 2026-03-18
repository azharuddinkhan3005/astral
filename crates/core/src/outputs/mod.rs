mod csv_output;
mod html;
mod json;
mod jsonl;
mod markdown;
mod sarif;
mod vector;

use crate::{AnalysisResult, Config};
use anyhow::Result;

/// Render analysis results to the specified format string.
pub fn render(results: &[AnalysisResult], format: &str, config: &Config) -> Result<String> {
    match format {
        "markdown" | "md" => markdown::render(results, config),
        "json" => json::render(results),
        "csv" => csv_output::render(results),
        "html" => html::render(results, config),
        "sarif" => sarif::render(results),
        "jsonl" => jsonl::render(results),
        "vector" | "vector-json" => vector::render(results),
        other => anyhow::bail!("Unsupported output format: {}", other),
    }
}

/// Write results to files in all configured output formats.
pub fn write_all_outputs(results: &[AnalysisResult], config: &Config) -> Result<Vec<String>> {
    let output_dir = &config.output_dir;
    std::fs::create_dir_all(output_dir)?;

    let mut written = Vec::new();

    for format in &config.outputs {
        let content = render(results, format, config)?;
        let ext = match format.as_str() {
            "markdown" | "md" => "md",
            "json" => "json",
            "csv" => "csv",
            "html" => "html",
            "sarif" => "sarif.json",
            "jsonl" => "jsonl",
            "vector" | "vector-json" => "vector.json",
            other => other,
        };
        let path = format!("{}/astral-report.{}", output_dir, ext);
        std::fs::write(&path, &content)?;
        written.push(path);
    }

    Ok(written)
}
