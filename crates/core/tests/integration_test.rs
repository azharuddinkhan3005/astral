use astral_core::{AnalysisResult, ChunkType, Config, CoreAnalyser, ResultStatus};
use std::path::Path;

/// Helper: workspace root (two levels up from CARGO_MANIFEST_DIR which is crates/core).
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

/// Helper: build a CoreAnalyser with default config.
fn default_analyser() -> CoreAnalyser {
    CoreAnalyser::new(Config::default())
}

/// Helper: build sample AnalysisResult entries for output-format tests.
fn sample_results() -> Vec<AnalysisResult> {
    vec![
        AnalysisResult {
            chunk_id: "id_a".to_string(),
            file_path: "src/main.ts".to_string(),
            chunk_type: ChunkType::Function,
            name: Some("greet".to_string()),
            analysis: "This function greets the user by name.".to_string(),
            status: ResultStatus::Succeeded,
        },
        AnalysisResult {
            chunk_id: "id_b".to_string(),
            file_path: "src/utils.ts".to_string(),
            chunk_type: ChunkType::Class,
            name: Some("Calculator".to_string()),
            analysis: "A stateful calculator class.".to_string(),
            status: ResultStatus::Succeeded,
        },
    ]
}

// ---------------------------------------------------------------------------
// 1. test_scan_typescript_fixture
// ---------------------------------------------------------------------------

#[test]
fn test_scan_typescript_fixture() {
    let fixture = workspace_root().join("tests/fixtures/sample-ts");
    let analyser = default_analyser();
    let chunks = analyser.scan(fixture.to_str().unwrap()).unwrap();

    // The sample-ts fixture has greeter.ts and logger.ts — both contain
    // functions and classes, so we expect several chunks.
    assert!(
        chunks.len() >= 3,
        "Expected at least 3 chunks from sample-ts, got {}",
        chunks.len()
    );

    let names: Vec<Option<&str>> = chunks.iter().map(|c| c.name.as_deref()).collect();
    // greeter.ts exports: greet (function), Greeter (class)
    assert!(
        names.contains(&Some("greet")),
        "Expected 'greet' in chunk names: {:?}",
        names
    );
    assert!(
        names.contains(&Some("Greeter")),
        "Expected 'Greeter' in chunk names: {:?}",
        names
    );
    // logger.ts exports: ConsoleLogger (class), createLogger (function)
    assert!(
        names.contains(&Some("ConsoleLogger")),
        "Expected 'ConsoleLogger' in chunk names: {:?}",
        names
    );
    assert!(
        names.contains(&Some("createLogger")),
        "Expected 'createLogger' in chunk names: {:?}",
        names
    );

    // All chunks should be typescript
    for chunk in &chunks {
        assert_eq!(chunk.language, "typescript");
    }
}

// ---------------------------------------------------------------------------
// 2. test_scan_python_fixture
// ---------------------------------------------------------------------------

#[test]
fn test_scan_python_fixture() {
    let fixture = workspace_root().join("tests/fixtures/sample-python");
    let analyser = default_analyser();
    let chunks = analyser.scan(fixture.to_str().unwrap()).unwrap();

    // calculator.py has: add, multiply, divide (functions) + Calculator (class)
    assert!(
        chunks.len() >= 2,
        "Expected at least 2 chunks from sample-python, got {}",
        chunks.len()
    );

    let names: Vec<Option<&str>> = chunks.iter().map(|c| c.name.as_deref()).collect();
    assert!(
        names.contains(&Some("Calculator")),
        "Expected 'Calculator' in chunk names: {:?}",
        names
    );

    for chunk in &chunks {
        assert_eq!(chunk.language, "python");
    }
}

// ---------------------------------------------------------------------------
// 3. test_full_pipeline — scan -> build_requests -> verify structure
// ---------------------------------------------------------------------------

#[test]
fn test_full_pipeline() {
    let fixture = workspace_root().join("tests/fixtures/sample-ts");
    let analyser = default_analyser();
    let chunks = analyser.scan(fixture.to_str().unwrap()).unwrap();
    assert!(!chunks.is_empty(), "Scan should return at least one chunk");

    let requests = analyser.build_requests(&chunks);
    assert_eq!(requests.len(), chunks.len(), "One request per chunk");

    // Verify every request is well-formed
    let chunk_ids: Vec<&str> = chunks.iter().map(|c| c.id.as_str()).collect();
    for req in &requests {
        // custom_id matches a chunk id
        assert!(
            chunk_ids.contains(&req.custom_id.as_str()),
            "Request custom_id '{}' not found in chunk ids",
            req.custom_id
        );
        // Model is set
        assert_eq!(req.params.model, analyser.config.model);
        // System prompt present
        assert!(req.params.system.is_some(), "System prompt should be set");
        assert!(
            !req.params.system.as_ref().unwrap().is_empty(),
            "System prompt should not be empty"
        );
        // Exactly one user message
        assert_eq!(req.params.messages.len(), 1);
        assert_eq!(req.params.messages[0].role, "user");
        assert!(
            !req.params.messages[0].content.is_empty(),
            "User message content should not be empty"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. test_aggregate_roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_aggregate_roundtrip() {
    let fixture = workspace_root().join("tests/fixtures/sample-ts");
    let analyser = default_analyser();
    let chunks = analyser.scan(fixture.to_str().unwrap()).unwrap();
    let requests = analyser.build_requests(&chunks);

    // Build mock JSONL results that mirror the request custom_ids
    let mut jsonl_lines = Vec::new();
    for req in &requests {
        let line = serde_json::json!({
            "custom_id": req.custom_id,
            "result": {
                "type": "succeeded",
                "message": {
                    "content": [{
                        "type": "text",
                        "text": format!("Analysis of {}", req.custom_id)
                    }]
                }
            }
        });
        jsonl_lines.push(serde_json::to_string(&line).unwrap());
    }
    let raw_jsonl = jsonl_lines.join("\n");

    let results = analyser.aggregate_results(&raw_jsonl, &chunks).unwrap();

    // Same count as chunks / requests
    assert_eq!(results.len(), chunks.len());

    // Every result maps back to a known chunk
    let chunk_ids: Vec<&str> = chunks.iter().map(|c| c.id.as_str()).collect();
    for result in &results {
        assert!(
            chunk_ids.contains(&result.chunk_id.as_str()),
            "Result chunk_id '{}' not in original chunks",
            result.chunk_id
        );
        assert_eq!(result.status, ResultStatus::Succeeded);
        assert!(
            result.analysis.contains("Analysis of"),
            "Analysis text should come from mock JSONL"
        );
        // file_path should be filled from the original chunk
        assert!(
            !result.file_path.contains("unknown"),
            "file_path should not be 'unknown' for known chunk ids"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. test_all_output_formats
// ---------------------------------------------------------------------------

#[test]
fn test_all_output_formats() {
    let results = sample_results();
    let config = Config::default();
    let analyser = CoreAnalyser::new(config);

    // Markdown
    let md = analyser.render_output(&results, "markdown").unwrap();
    assert!(!md.is_empty(), "Markdown output should not be empty");
    assert!(md.contains('#'), "Markdown should contain heading markers");
    assert!(md.contains("greet"), "Markdown should mention chunk names");

    // JSON
    let json_str = analyser.render_output(&results, "json").unwrap();
    assert!(!json_str.is_empty(), "JSON output should not be empty");
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("JSON output should be valid JSON");
    assert!(
        parsed.get("results").is_some(),
        "JSON should contain 'results' key"
    );
    assert!(
        parsed.get("stats").is_some(),
        "JSON should contain 'stats' key"
    );

    // CSV
    let csv_str = analyser.render_output(&results, "csv").unwrap();
    assert!(!csv_str.is_empty(), "CSV output should not be empty");
    assert!(csv_str.contains(','), "CSV output should contain commas");
    assert!(csv_str.contains("chunk_id"), "CSV should have a header row");

    // HTML
    let html_str = analyser.render_output(&results, "html").unwrap();
    assert!(!html_str.is_empty(), "HTML output should not be empty");
    assert!(
        html_str.contains("<html"),
        "HTML output should contain <html> tag"
    );
    assert!(
        html_str.contains("Astral Analysis Report"),
        "HTML should contain the report title"
    );
}

// ---------------------------------------------------------------------------
// 6. test_config_from_json
// ---------------------------------------------------------------------------

#[test]
fn test_config_from_json() {
    // Minimal JSON — all fields should get defaults
    let json_minimal = "{}";
    let config: Config = serde_json::from_str(json_minimal).unwrap();
    assert_eq!(config.chunk_by, "function");
    assert_eq!(config.model, "claude-haiku-4-5-20251001");
    assert_eq!(config.max_tokens, 512);
    assert!(config.include.is_empty());
    assert!(config.exclude.is_empty());

    // Partial overrides
    let json_partial = r#"{"model": "claude-sonnet-4-20250514", "max_tokens": 1024}"#;
    let config2: Config = serde_json::from_str(json_partial).unwrap();
    assert_eq!(config2.model, "claude-sonnet-4-20250514");
    assert_eq!(config2.max_tokens, 1024);
    // Non-overridden fields keep defaults
    assert_eq!(config2.chunk_by, "function");

    // Full config
    let json_full = r#"{
        "include": ["src/**/*.ts"],
        "exclude": ["**/*.test.ts"],
        "chunk_by": "class",
        "model": "claude-opus-4-20250514",
        "max_tokens": 2048,
        "outputs": ["markdown", "json", "csv", "html"],
        "output_dir": "./custom-output"
    }"#;
    let config3: Config = serde_json::from_str(json_full).unwrap();
    assert_eq!(config3.include, vec!["src/**/*.ts"]);
    assert_eq!(config3.exclude, vec!["**/*.test.ts"]);
    assert_eq!(config3.chunk_by, "class");
    assert_eq!(config3.model, "claude-opus-4-20250514");
    assert_eq!(config3.max_tokens, 2048);
    assert_eq!(config3.outputs.len(), 4);
    assert_eq!(config3.output_dir, "./custom-output");
}

// ---------------------------------------------------------------------------
// 7. test_chunk_determinism
// ---------------------------------------------------------------------------

#[test]
fn test_chunk_determinism() {
    let fixture = workspace_root().join("tests/fixtures/sample-ts");
    let analyser = default_analyser();

    let chunks1 = analyser.scan(fixture.to_str().unwrap()).unwrap();
    let chunks2 = analyser.scan(fixture.to_str().unwrap()).unwrap();

    assert_eq!(
        chunks1.len(),
        chunks2.len(),
        "Two scans of the same input should produce the same number of chunks"
    );

    let ids1: Vec<&str> = chunks1.iter().map(|c| c.id.as_str()).collect();
    let ids2: Vec<&str> = chunks2.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids1, ids2, "Chunk IDs should be identical across two scans");

    // Also verify names match
    let names1: Vec<Option<&str>> = chunks1.iter().map(|c| c.name.as_deref()).collect();
    let names2: Vec<Option<&str>> = chunks2.iter().map(|c| c.name.as_deref()).collect();
    assert_eq!(
        names1, names2,
        "Chunk names should be identical across two scans"
    );
}
