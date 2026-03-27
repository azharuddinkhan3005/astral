use crate::{AnalysisResult, ChunkType, ResultStatus};
use anyhow::Result;
use serde::Serialize;

/// Render analysis results as a SARIF 2.1.0 JSON document.
///
/// SARIF (Static Analysis Results Interchange Format) is an OASIS standard
/// that renders natively in the GitHub PR Security tab.
pub fn render(results: &[AnalysisResult]) -> Result<String> {
    let mut rules = Vec::new();
    let mut sarif_results = Vec::new();

    // Collect unique rules from chunk_types.
    let mut seen_rules = std::collections::HashSet::new();

    for result in results {
        let rule_id = rule_id_for(&result.chunk_type);

        if seen_rules.insert(rule_id.clone()) {
            rules.push(SarifRule {
                id: rule_id.clone(),
                short_description: SarifMessage {
                    text: rule_description(&result.chunk_type),
                },
            });
        }

        sarif_results.push(SarifResult {
            rule_id: rule_id.clone(),
            message: SarifMessage {
                text: result.analysis.clone(),
            },
            level: sarif_level(&result.status),
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: result.file_path.clone(),
                    },
                },
            }],
        });
    }

    let report = SarifReport {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "astral".to_string(),
                    version: "0.1.0".to_string(),
                    information_uri: "https://github.com/anthropics/astral".to_string(),
                    rules,
                },
            },
            results: sarif_results,
        }],
    };

    Ok(serde_json::to_string_pretty(&report)?)
}

fn rule_id_for(chunk_type: &ChunkType) -> String {
    match chunk_type {
        ChunkType::Function => "astral/function-review".to_string(),
        ChunkType::Class => "astral/class-review".to_string(),
        ChunkType::Module => "astral/module-review".to_string(),
        ChunkType::Block => "astral/block-review".to_string(),
    }
}

fn rule_description(chunk_type: &ChunkType) -> String {
    match chunk_type {
        ChunkType::Function => "Analysis of a function chunk".to_string(),
        ChunkType::Class => "Analysis of a class chunk".to_string(),
        ChunkType::Module => "Analysis of a module chunk".to_string(),
        ChunkType::Block => "Analysis of a code block chunk".to_string(),
    }
}

fn sarif_level(status: &ResultStatus) -> String {
    match status {
        ResultStatus::Succeeded => "note".to_string(),
        ResultStatus::Errored => "error".to_string(),
        ResultStatus::Canceled => "warning".to_string(),
        ResultStatus::Expired => "warning".to_string(),
    }
}

// ---------------------------------------------------------------------------
// SARIF 2.1.0 serde structs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SarifReport {
    #[serde(rename = "$schema")]
    schema: String,
    version: String,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: String,
    version: String,
    information_uri: String,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: String,
    short_description: SarifMessage,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    message: SarifMessage,
    level: String,
    locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
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
    fn test_sarif_render_valid_json() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["version"], "2.1.0");
        assert!(
            parsed["$schema"]
                .as_str()
                .unwrap()
                .contains("sarif-schema-2.1.0")
        );
    }

    #[test]
    fn test_sarif_tool_metadata() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let driver = &parsed["runs"][0]["tool"]["driver"];
        assert_eq!(driver["name"], "astral");
        assert_eq!(driver["version"], "0.1.0");
        assert_eq!(
            driver["informationUri"],
            "https://github.com/anthropics/astral"
        );
    }

    #[test]
    fn test_sarif_rules_deduplication() {
        let results = vec![
            AnalysisResult {
                chunk_id: "a".to_string(),
                file_path: "a.ts".to_string(),
                chunk_type: ChunkType::Function,
                name: None,
                analysis: "first".to_string(),
                status: ResultStatus::Succeeded,
            },
            AnalysisResult {
                chunk_id: "b".to_string(),
                file_path: "b.ts".to_string(),
                chunk_type: ChunkType::Function,
                name: None,
                analysis: "second".to_string(),
                status: ResultStatus::Succeeded,
            },
        ];

        let output = render(&results).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        // Only one rule for two Function chunks
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["id"], "astral/function-review");
    }

    #[test]
    fn test_sarif_results_count() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_sarif_result_fields() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let first = &parsed["runs"][0]["results"][0];
        assert_eq!(first["ruleId"], "astral/function-review");
        assert_eq!(first["message"]["text"], "Greets the user.");
        assert_eq!(first["level"], "note");
        assert_eq!(
            first["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/main.ts"
        );
    }

    #[test]
    fn test_sarif_error_level() {
        let output = render(&sample_results()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let second = &parsed["runs"][0]["results"][1];
        assert_eq!(second["ruleId"], "astral/class-review");
        assert_eq!(second["level"], "error");
    }

    #[test]
    fn test_sarif_empty_results() {
        let output = render(&[]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert!(results.is_empty());

        let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert!(rules.is_empty());
    }
}
