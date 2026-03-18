use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JS-visible data types
// ---------------------------------------------------------------------------

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsCodeChunk {
    pub id: String,
    pub file_path: String,
    pub language: String,
    pub chunk_type: String,
    pub name: Option<String>,
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub imports: Vec<String>,
}

impl From<astral_core::CodeChunk> for JsCodeChunk {
    fn from(c: astral_core::CodeChunk) -> Self {
        Self {
            id: c.id,
            file_path: c.file_path,
            language: c.language,
            chunk_type: format!("{:?}", c.chunk_type).to_lowercase(),
            name: c.name,
            content: c.content,
            start_line: c.start_line as u32,
            end_line: c.end_line as u32,
            imports: c.imports,
        }
    }
}

impl From<JsCodeChunk> for astral_core::CodeChunk {
    fn from(c: JsCodeChunk) -> Self {
        let chunk_type = match c.chunk_type.as_str() {
            "function" => astral_core::ChunkType::Function,
            "class" => astral_core::ChunkType::Class,
            "module" => astral_core::ChunkType::Module,
            _ => astral_core::ChunkType::Block,
        };
        Self {
            id: c.id,
            file_path: c.file_path,
            language: c.language,
            chunk_type,
            name: c.name,
            content: c.content,
            start_line: c.start_line as usize,
            end_line: c.end_line as usize,
            imports: c.imports,
        }
    }
}

// ---------------------------------------------------------------------------
// Analyser — thin wrapper around astral_core::CoreAnalyser
// ---------------------------------------------------------------------------

#[napi]
pub struct Analyser {
    inner: astral_core::CoreAnalyser,
}

#[napi]
impl Analyser {
    /// Create a new Analyser from a JSON config string.
    /// Pass `"{}"` for defaults.
    #[napi(constructor)]
    pub fn new(config_json: String) -> Result<Self> {
        let config: astral_core::Config = serde_json::from_str(&config_json)
            .map_err(|e| Error::from_reason(format!("Invalid config JSON: {e}")))?;
        Ok(Self {
            inner: astral_core::CoreAnalyser::new(config),
        })
    }

    /// Walk + parse + chunk the repository at `repo_path`.
    /// Returns an array of code chunk objects.
    #[napi]
    pub fn scan(&self, repo_path: String) -> Result<Vec<JsCodeChunk>> {
        let chunks = self
            .inner
            .scan(&repo_path)
            .map_err(|e| Error::from_reason(format!("Scan failed: {e}")))?;
        Ok(chunks.into_iter().map(JsCodeChunk::from).collect())
    }

    /// Build Anthropic Batch API request bodies from a repository scan.
    /// Returns a JSON string containing an array of `BatchRequest` objects.
    #[napi]
    pub fn build_requests(&self, repo_path: String) -> Result<String> {
        let chunks = self
            .inner
            .scan(&repo_path)
            .map_err(|e| Error::from_reason(format!("Scan failed: {e}")))?;
        let requests = self.inner.build_requests(&chunks);
        let json = serde_json::to_string(&requests)
            .map_err(|e| Error::from_reason(format!("Serialisation failed: {e}")))?;
        Ok(json)
    }

    /// Aggregate raw JSONL results (as returned by the Batch API) back into
    /// structured analysis results.  Requires a prior `scan` to map chunk IDs.
    /// Returns a JSON string of `AnalysisResult[]`.
    #[napi]
    pub fn aggregate_results(&self, repo_path: String, jsonl: String) -> Result<String> {
        let chunks = self
            .inner
            .scan(&repo_path)
            .map_err(|e| Error::from_reason(format!("Scan failed: {e}")))?;
        let results = self
            .inner
            .aggregate_results(&jsonl, &chunks)
            .map_err(|e| Error::from_reason(format!("Aggregation failed: {e}")))?;
        let json = serde_json::to_string(&results)
            .map_err(|e| Error::from_reason(format!("Serialisation failed: {e}")))?;
        Ok(json)
    }

    /// Render analysis results (JSON string) to the given output format
    /// (e.g. `"markdown"`, `"json"`, `"csv"`, `"html"`).
    #[napi]
    pub fn render_output(&self, results_json: String, format: String) -> Result<String> {
        let results: Vec<astral_core::AnalysisResult> = serde_json::from_str(&results_json)
            .map_err(|e| Error::from_reason(format!("Invalid results JSON: {e}")))?;
        let rendered = self
            .inner
            .render_output(&results, &format)
            .map_err(|e| Error::from_reason(format!("Render failed: {e}")))?;
        Ok(rendered)
    }
}
