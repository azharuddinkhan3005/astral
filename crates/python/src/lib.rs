// PyO3 0.22 macros trigger these lints; safe to suppress until PyO3 update
#![allow(clippy::useless_conversion, unsafe_op_in_unsafe_fn)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use astral_core::{Config, CoreAnalyser};

/// Convert an anyhow error into a PyRuntimeError.
fn to_py_err(e: impl std::fmt::Display) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
}

// ---------------------------------------------------------------------------
// Analyser pyclass — thin wrapper around astral_core::CoreAnalyser
// ---------------------------------------------------------------------------

#[pyclass]
struct Analyser {
    inner: CoreAnalyser,
}

#[pymethods]
impl Analyser {
    /// Create a new Analyser from a JSON configuration string.
    #[new]
    fn new(config_json: &str) -> PyResult<Self> {
        let config: Config = serde_json::from_str(config_json).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid config JSON: {e}"))
        })?;
        Ok(Self {
            inner: CoreAnalyser::new(config),
        })
    }

    /// Walk the repo, parse files, chunk them, and return a list of dicts.
    fn scan<'py>(&self, py: Python<'py>, repo_path: &str) -> PyResult<Bound<'py, PyList>> {
        let chunks = self.inner.scan(repo_path).map_err(to_py_err)?;

        let list = PyList::empty_bound(py);
        for chunk in &chunks {
            let dict = PyDict::new_bound(py);
            dict.set_item("id", &chunk.id)?;
            dict.set_item("file_path", &chunk.file_path)?;
            dict.set_item("language", &chunk.language)?;
            dict.set_item("chunk_type", format!("{:?}", chunk.chunk_type))?;
            dict.set_item("name", &chunk.name)?;
            dict.set_item("content", &chunk.content)?;
            dict.set_item("start_line", chunk.start_line)?;
            dict.set_item("end_line", chunk.end_line)?;
            dict.set_item("imports", &chunk.imports)?;
            list.append(dict)?;
        }
        Ok(list)
    }

    /// Build batch requests from a repo path and return them as a JSON string.
    fn build_requests(&self, repo_path: &str) -> PyResult<String> {
        let chunks = self.inner.scan(repo_path).map_err(to_py_err)?;
        let requests = self.inner.build_requests(&chunks);
        serde_json::to_string(&requests).map_err(to_py_err)
    }

    /// Aggregate raw JSONL results from the Batch API.
    fn aggregate_results(&self, jsonl: &str, repo_path: &str) -> PyResult<String> {
        let chunks = self.inner.scan(repo_path).map_err(to_py_err)?;
        let results = self
            .inner
            .aggregate_results(jsonl, &chunks)
            .map_err(to_py_err)?;
        serde_json::to_string(&results).map_err(to_py_err)
    }

    /// Render analysis results to the specified output format.
    fn render_output(&self, results_json: &str, format: &str) -> PyResult<String> {
        let results: Vec<astral_core::AnalysisResult> = serde_json::from_str(results_json)
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid results JSON: {e}"
                ))
            })?;
        self.inner
            .render_output(&results, format)
            .map_err(to_py_err)
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

#[pymodule]
fn astral(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Analyser>()?;
    Ok(())
}
