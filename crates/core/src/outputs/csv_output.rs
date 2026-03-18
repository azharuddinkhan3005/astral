use crate::AnalysisResult;
use anyhow::Result;

/// Render analysis results as CSV.
pub fn render(results: &[AnalysisResult]) -> Result<String> {
    let mut wtr = csv::Writer::from_writer(Vec::new());

    // Header
    wtr.write_record([
        "chunk_id",
        "file_path",
        "chunk_type",
        "name",
        "status",
        "analysis",
    ])?;

    for result in results {
        wtr.write_record([
            &result.chunk_id,
            &result.file_path,
            &format!("{:?}", result.chunk_type),
            result.name.as_deref().unwrap_or(""),
            &format!("{:?}", result.status),
            &result.analysis,
        ])?;
    }

    let bytes = wtr.into_inner()?;
    Ok(String::from_utf8(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, ResultStatus};

    #[test]
    fn test_csv_render() {
        let results = vec![AnalysisResult {
            chunk_id: "a".to_string(),
            file_path: "src/main.ts".to_string(),
            chunk_type: ChunkType::Function,
            name: Some("greet".to_string()),
            analysis: "Greets the user.".to_string(),
            status: ResultStatus::Succeeded,
        }];

        let output = render(&results).unwrap();
        assert!(output.contains("chunk_id,file_path,chunk_type,name,status,analysis"));
        assert!(output.contains("src/main.ts"));
        assert!(output.contains("greet"));
    }
}
