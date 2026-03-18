pub mod agent;
pub mod aggregator;
pub mod batch_builder;
pub mod chunker;
pub mod outputs;
pub mod parser;
pub mod walker;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub id: String,
    pub file_path: String,
    pub language: String,
    pub chunk_type: ChunkType,
    pub name: Option<String>,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Function,
    Class,
    Module,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    pub custom_id: String,
    pub params: BatchRequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequestParams {
    pub model: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub chunk_id: String,
    pub file_path: String,
    pub chunk_type: ChunkType,
    pub name: Option<String>,
    pub analysis: String,
    pub status: ResultStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Succeeded,
    Errored,
    Canceled,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisMode {
    Summarise,
    Dependencies,
    CodeReview,
    TestGeneration,
    SecurityAudit,
    DocGeneration,
    Custom(String),
}

impl AnalysisMode {
    pub fn system_prompt(&self) -> String {
        match self {
            Self::Summarise => {
                "You are a code analyst. For the given code chunk, provide a concise summary \
                 of what it does, its inputs, outputs, and any notable patterns or concerns. \
                 Be specific and technical."
                    .to_string()
            }
            Self::Dependencies => {
                "You are a dependency analyst. For the given code chunk, identify: \
                 1) What external modules/functions it depends on \
                 2) What other parts of the codebase likely depend on it \
                 3) The coupling level (tight/loose) and suggestions for improvement."
                    .to_string()
            }
            Self::CodeReview => "You are a senior code reviewer. Review the given code chunk for: \
                 1) Bugs and logic errors \
                 2) Code smells and anti-patterns \
                 3) Missing error handling \
                 4) Performance concerns \
                 5) Suggestions for improvement. \
                 Be specific with line references."
                .to_string(),
            Self::TestGeneration => {
                "You are a test engineer. For the given code chunk, generate comprehensive \
                 unit tests that cover: happy paths, edge cases, error conditions, and \
                 boundary values. Use the appropriate test framework for the language. \
                 Output only the test code."
                    .to_string()
            }
            Self::SecurityAudit => "You are a security auditor. Analyse the given code chunk for: \
                 1) Injection vulnerabilities (SQL, command, XSS) \
                 2) Authentication/authorization issues \
                 3) Secrets or credentials in code \
                 4) Unsafe patterns \
                 5) OWASP Top 10 vulnerabilities. \
                 Rate severity: critical/high/medium/low/info."
                .to_string(),
            Self::DocGeneration => {
                "You are a documentation writer. Generate language-appropriate inline \
                 documentation for the given code chunk: \
                 - TypeScript/JavaScript: JSDoc /** */ blocks \
                 - Python: Google-style docstrings \
                 - Rust: /// rustdoc comments \
                 - Java: Javadoc /** */ blocks \
                 Output the documented code."
                    .to_string()
            }
            Self::Custom(prompt) => prompt.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default = "default_chunk_by")]
    pub chunk_by: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_analysis_mode")]
    pub analysis_mode: AnalysisMode,
    #[serde(default = "default_outputs")]
    pub outputs: Vec<String>,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

fn default_chunk_by() -> String {
    "function".to_string()
}
fn default_model() -> String {
    "claude-haiku-4-5-20251001".to_string()
}
fn default_max_tokens() -> u32 {
    512
}
fn default_analysis_mode() -> AnalysisMode {
    AnalysisMode::Summarise
}
fn default_outputs() -> Vec<String> {
    vec!["markdown".to_string(), "json".to_string()]
}
fn default_output_dir() -> String {
    "./astral-output".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            include: vec![],
            exclude: vec![],
            chunk_by: default_chunk_by(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            analysis_mode: default_analysis_mode(),
            outputs: default_outputs(),
            output_dir: default_output_dir(),
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a deterministic chunk ID: sha256(file_path + start_line)
pub fn chunk_id(file_path: &str, start_line: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}{}", file_path, start_line));
    format!("{:x}", hasher.finalize())
}

/// Detect language from file extension. Returns None for unsupported extensions.
pub fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "ts" | "tsx" => Some("typescript".to_string()),
        "js" | "jsx" => Some("javascript".to_string()),
        "py" => Some("python".to_string()),
        "java" => Some("java".to_string()),
        "go" => Some("go".to_string()),
        "rs" => Some("rust".to_string()),
        "rb" => Some("ruby".to_string()),
        "php" => Some("php".to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Core analyser (library entry point)
// ---------------------------------------------------------------------------

pub struct CoreAnalyser {
    pub config: Config,
}

impl CoreAnalyser {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn from_config_file(path: &str) -> anyhow::Result<Self> {
        let config = Config::from_file(path)?;
        Ok(Self { config })
    }

    /// Walk the repo, parse files, chunk them, and return all chunks.
    pub fn scan(&self, repo_path: &str) -> anyhow::Result<Vec<CodeChunk>> {
        let files = walker::walk_repo(repo_path, &self.config)?;
        let chunks = chunker::chunk_files(&files, &self.config, repo_path)?;
        Ok(chunks)
    }

    /// Build batch requests from chunks.
    pub fn build_requests(&self, chunks: &[CodeChunk]) -> Vec<BatchRequest> {
        batch_builder::build_batch_requests(chunks, &self.config)
    }

    /// Aggregate raw JSONL results back to AnalysisResults.
    pub fn aggregate_results(
        &self,
        raw_results: &str,
        chunks: &[CodeChunk],
    ) -> anyhow::Result<Vec<AnalysisResult>> {
        aggregator::aggregate(raw_results, chunks)
    }

    /// Render results to the specified format.
    pub fn render_output(
        &self,
        results: &[AnalysisResult],
        format: &str,
    ) -> anyhow::Result<String> {
        outputs::render(results, format, &self.config)
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum AstralError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("Parse error in {file}: {message}")]
    ParseError { file: String, message: String },
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("Output format not supported: {0}")]
    UnsupportedOutputFormat(String),
    #[error("Aggregation error: {0}")]
    AggregationError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
