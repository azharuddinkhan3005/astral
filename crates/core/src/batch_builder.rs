use crate::{BatchRequest, BatchRequestParams, CodeChunk, Config, Message};

/// Build BatchRequest structs from CodeChunks, ready for Anthropic Batch API submission.
pub fn build_batch_requests(chunks: &[CodeChunk], config: &Config) -> Vec<BatchRequest> {
    let system_prompt = config.analysis_mode.system_prompt();

    chunks
        .iter()
        .map(|chunk| {
            let user_content = format_chunk_prompt(chunk);

            BatchRequest {
                custom_id: chunk.id.clone(),
                params: BatchRequestParams {
                    model: config.model.clone(),
                    max_tokens: config.max_tokens,
                    system: Some(system_prompt.clone()),
                    messages: vec![Message {
                        role: "user".to_string(),
                        content: user_content,
                    }],
                },
            }
        })
        .collect()
}

/// Format a code chunk into a prompt for the batch API.
fn format_chunk_prompt(chunk: &CodeChunk) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("File: {}\n", chunk.file_path));
    prompt.push_str(&format!("Language: {}\n", chunk.language));
    prompt.push_str(&format!("Type: {:?}\n", chunk.chunk_type));
    if let Some(name) = &chunk.name {
        prompt.push_str(&format!("Name: {}\n", name));
    }
    prompt.push_str(&format!("Lines: {}-{}\n", chunk.start_line, chunk.end_line));

    if !chunk.imports.is_empty() {
        prompt.push_str("\nImports:\n");
        for import in &chunk.imports {
            prompt.push_str(&format!("  {}\n", import));
        }
    }

    prompt.push_str(&format!(
        "\nCode:\n```{}\n{}\n```\n",
        chunk.language, chunk.content
    ));

    prompt
}

/// Estimate the cost of a batch of requests in USD.
/// Uses token estimation: content.length / 4 characters per token.
pub fn estimate_cost(requests: &[BatchRequest]) -> CostEstimate {
    let mut total_input_tokens: u64 = 0;
    let mut total_max_output_tokens: u64 = 0;

    for req in requests {
        // Estimate input tokens from system + messages
        let mut input_chars: usize = 0;
        if let Some(sys) = &req.params.system {
            input_chars += sys.len();
        }
        for msg in &req.params.messages {
            input_chars += msg.content.len();
        }
        total_input_tokens += (input_chars / 4) as u64;
        total_max_output_tokens += req.params.max_tokens as u64;
    }

    let (input_cost_per_m, output_cost_per_m) = model_pricing(
        requests
            .first()
            .map(|r| r.params.model.as_str())
            .unwrap_or("claude-haiku-4-5-20251001"),
    );

    // Batch API is 50% cheaper
    let batch_discount = 0.5;
    let input_cost = (total_input_tokens as f64 / 1_000_000.0) * input_cost_per_m * batch_discount;
    let output_cost =
        (total_max_output_tokens as f64 / 1_000_000.0) * output_cost_per_m * batch_discount;

    CostEstimate {
        request_count: requests.len(),
        estimated_input_tokens: total_input_tokens,
        estimated_max_output_tokens: total_max_output_tokens,
        estimated_cost_usd: input_cost + output_cost,
        model: requests
            .first()
            .map(|r| r.params.model.clone())
            .unwrap_or_default(),
    }
}

/// Get pricing per 1M tokens for a model (input, output).
fn model_pricing(model: &str) -> (f64, f64) {
    if model.contains("opus") {
        (5.0, 25.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else {
        // haiku default
        (0.80, 4.0)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CostEstimate {
    pub request_count: usize,
    pub estimated_input_tokens: u64,
    pub estimated_max_output_tokens: u64,
    pub estimated_cost_usd: f64,
    pub model: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, CodeChunk};

    fn sample_chunks() -> Vec<CodeChunk> {
        vec![
            CodeChunk {
                id: "abc123".to_string(),
                file_path: "src/main.ts".to_string(),
                language: "typescript".to_string(),
                chunk_type: ChunkType::Function,
                name: Some("greet".to_string()),
                content: "function greet(name: string) { return `Hello, ${name}`; }".to_string(),
                start_line: 1,
                end_line: 3,
                imports: vec!["import { foo } from './foo';".to_string()],
            },
            CodeChunk {
                id: "def456".to_string(),
                file_path: "src/utils.ts".to_string(),
                language: "typescript".to_string(),
                chunk_type: ChunkType::Function,
                name: Some("add".to_string()),
                content: "function add(a: number, b: number) { return a + b; }".to_string(),
                start_line: 1,
                end_line: 1,
                imports: vec![],
            },
        ]
    }

    #[test]
    fn test_build_batch_requests() {
        let chunks = sample_chunks();
        let config = Config::default();
        let requests = build_batch_requests(&chunks, &config);

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].custom_id, "abc123");
        assert_eq!(requests[1].custom_id, "def456");
        assert_eq!(requests[0].params.model, config.model);
        assert_eq!(requests[0].params.max_tokens, config.max_tokens);
        assert!(requests[0].params.system.is_some());
        assert_eq!(requests[0].params.messages.len(), 1);
        assert_eq!(requests[0].params.messages[0].role, "user");
    }

    #[test]
    fn test_batch_request_contains_chunk_content() {
        let chunks = sample_chunks();
        let config = Config::default();
        let requests = build_batch_requests(&chunks, &config);

        let content = &requests[0].params.messages[0].content;
        assert!(content.contains("greet"));
        assert!(content.contains("src/main.ts"));
        assert!(content.contains("typescript"));
    }

    #[test]
    fn test_cost_estimate() {
        let chunks = sample_chunks();
        let config = Config::default();
        let requests = build_batch_requests(&chunks, &config);
        let estimate = estimate_cost(&requests);

        assert_eq!(estimate.request_count, 2);
        assert!(estimate.estimated_input_tokens > 0);
        assert!(estimate.estimated_cost_usd > 0.0);
        assert_eq!(estimate.model, config.model);
    }

    #[test]
    fn test_format_chunk_prompt_includes_metadata() {
        let chunk = &sample_chunks()[0];
        let prompt = format_chunk_prompt(chunk);

        assert!(prompt.contains("File: src/main.ts"));
        assert!(prompt.contains("Language: typescript"));
        assert!(prompt.contains("Name: greet"));
        assert!(prompt.contains("Lines: 1-3"));
        assert!(prompt.contains("Imports:"));
        assert!(prompt.contains("```typescript"));
    }

    #[test]
    fn test_empty_chunks_returns_empty_requests() {
        let config = Config::default();
        let requests = build_batch_requests(&[], &config);
        assert!(requests.is_empty());
    }
}
