use crate::parser::{chunk_node_types, parse_source};
use crate::walker::FileEntry;
use crate::{chunk_id, ChunkType, CodeChunk, Config};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

/// Chunk all files into semantic code chunks using tree-sitter.
pub fn chunk_files(
    files: &[FileEntry],
    config: &Config,
    repo_path: &str,
) -> Result<Vec<CodeChunk>> {
    let base = Path::new(repo_path);

    // Use rayon for parallel file processing
    let chunks: Vec<Vec<CodeChunk>> = files
        .par_iter()
        .filter_map(|file| match chunk_file(file, &config.chunk_by, base) {
            Ok(chunks) => Some(chunks),
            Err(e) => {
                eprintln!("Warning: failed to chunk {}: {}", file.path, e);
                None
            }
        })
        .collect();

    Ok(chunks.into_iter().flatten().collect())
}

/// Chunk a single file into code chunks based on tree-sitter AST.
pub fn chunk_file(file: &FileEntry, chunk_by: &str, repo_base: &Path) -> Result<Vec<CodeChunk>> {
    let abs_path = repo_base.join(&file.path);
    let content = std::fs::read_to_string(&abs_path)?;

    chunk_source(&content, &file.path, &file.language, chunk_by)
}

/// Chunk source code string into semantic code chunks.
/// This is the core chunking logic, separated for testability.
pub fn chunk_source(
    source: &str,
    file_path: &str,
    language: &str,
    chunk_by: &str,
) -> Result<Vec<CodeChunk>> {
    let tree = parse_source(source, language)?;
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    let target_types = chunk_node_types(language);
    if target_types.is_empty() {
        // Fall back to treating the entire file as a single module chunk
        return Ok(vec![CodeChunk {
            id: chunk_id(file_path, 0),
            file_path: file_path.to_string(),
            language: language.to_string(),
            chunk_type: ChunkType::Module,
            name: Path::new(file_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string()),
            content: source.to_string(),
            start_line: 1,
            end_line: source.lines().count(),
            imports: extract_imports(source, language),
        }]);
    }

    let mut chunks = Vec::new();
    let mut cursor = root.walk();

    let ctx = ChunkContext {
        source_bytes,
        file_path,
        language,
        target_types,
        chunk_by,
    };
    collect_chunks(&mut cursor, &ctx, &mut chunks);

    // If no chunks found (e.g., file has no functions/classes), treat as module chunk
    if chunks.is_empty() {
        chunks.push(CodeChunk {
            id: chunk_id(file_path, 0),
            file_path: file_path.to_string(),
            language: language.to_string(),
            chunk_type: ChunkType::Module,
            name: Path::new(file_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string()),
            content: source.to_string(),
            start_line: 1,
            end_line: source.lines().count(),
            imports: extract_imports(source, language),
        });
    }

    Ok(chunks)
}

/// Context for recursive chunk collection — avoids too many function parameters.
struct ChunkContext<'a> {
    source_bytes: &'a [u8],
    file_path: &'a str,
    language: &'a str,
    target_types: &'a [&'a str],
    chunk_by: &'a str,
}

/// Recursively collect chunks from the AST.
fn collect_chunks(
    cursor: &mut tree_sitter::TreeCursor,
    ctx: &ChunkContext,
    chunks: &mut Vec<CodeChunk>,
) {
    let node = cursor.node();
    let node_type = node.kind();

    if ctx.target_types.contains(&node_type) {
        let start_line = node.start_position().row + 1; // 1-indexed
        let end_line = node.end_position().row + 1;
        let content = node.utf8_text(ctx.source_bytes).unwrap_or("").to_string();

        let name = extract_name(cursor, ctx.source_bytes);
        let chunk_type = classify_node(node_type, ctx.language);

        // Apply chunk_by filter
        let should_include = match ctx.chunk_by {
            "function" => chunk_type == ChunkType::Function || chunk_type == ChunkType::Class,
            "class" => chunk_type == ChunkType::Class,
            _ => true, // "all" or unknown
        };

        if should_include {
            chunks.push(CodeChunk {
                id: chunk_id(ctx.file_path, start_line),
                file_path: ctx.file_path.to_string(),
                language: ctx.language.to_string(),
                chunk_type,
                name,
                content,
                start_line,
                end_line,
                imports: Vec::new(), // imports are file-level, set later if needed
            });
        }
    }

    // Recurse into children
    if cursor.goto_first_child() {
        loop {
            collect_chunks(cursor, ctx, chunks);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Extract the name of a function/class from the AST node.
fn extract_name(cursor: &tree_sitter::TreeCursor, source_bytes: &[u8]) -> Option<String> {
    let node = cursor.node();

    // Look for a 'name' or 'identifier' child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let field_name = node.field_name_for_child(i as u32);
            if field_name == Some("name") {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
            if child.kind() == "identifier"
                || child.kind() == "property_identifier"
                || child.kind() == "type_identifier"
            {
                return child.utf8_text(source_bytes).ok().map(|s| s.to_string());
            }
        }
    }
    None
}

/// Classify a tree-sitter node type into our ChunkType enum.
fn classify_node(node_type: &str, _language: &str) -> ChunkType {
    match node_type {
        "function_declaration"
        | "function_definition"
        | "function_item"
        | "method_declaration"
        | "method_definition"
        | "method"
        | "arrow_function" => ChunkType::Function,
        "class_declaration" | "class_definition" | "class" | "impl_item" => ChunkType::Class,
        "export_statement" => ChunkType::Module,
        _ => ChunkType::Block,
    }
}

/// Extract import statements from source code (simple heuristic).
fn extract_imports(source: &str, language: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let is_import = match language {
            "typescript" | "javascript" | "tsx" | "jsx" => {
                trimmed.starts_with("import ") || trimmed.starts_with("require(")
            }
            "python" => trimmed.starts_with("import ") || trimmed.starts_with("from "),
            "java" => trimmed.starts_with("import "),
            "go" => trimmed.starts_with("import "),
            "rust" => trimmed.starts_with("use ") || trimmed.starts_with("extern crate"),
            "ruby" => trimmed.starts_with("require ") || trimmed.starts_with("require_relative"),
            "php" => {
                trimmed.starts_with("use ")
                    || trimmed.contains("require ")
                    || trimmed.contains("include ")
            }
            _ => false,
        };
        if is_import {
            imports.push(trimmed.to_string());
        }
    }
    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_typescript_functions() {
        let source = r#"
import { foo } from './foo';

function greet(name: string): string {
    return `Hello, ${name}`;
}

function add(a: number, b: number): number {
    return a + b;
}

class Calculator {
    multiply(a: number, b: number): number {
        return a * b;
    }
}
"#;
        let chunks = chunk_source(source, "test.ts", "typescript", "function").unwrap();
        assert!(
            chunks.len() >= 3,
            "Expected at least 3 chunks, got {}",
            chunks.len()
        );

        let names: Vec<Option<&str>> = chunks.iter().map(|c| c.name.as_deref()).collect();
        assert!(names.contains(&Some("greet")));
        assert!(names.contains(&Some("add")));
        assert!(names.contains(&Some("Calculator")));
    }

    #[test]
    fn test_chunk_python_functions() {
        let source = r#"
import os
from pathlib import Path

def hello(name):
    return f"Hello, {name}"

class Greeter:
    def greet(self, name):
        return f"Hi, {name}"
"#;
        let chunks = chunk_source(source, "test.py", "python", "function").unwrap();
        assert!(
            chunks.len() >= 2,
            "Expected at least 2 chunks, got {}",
            chunks.len()
        );

        let names: Vec<Option<&str>> = chunks.iter().map(|c| c.name.as_deref()).collect();
        assert!(names.contains(&Some("hello")));
        assert!(names.contains(&Some("Greeter")));
    }

    #[test]
    fn test_chunk_rust_functions() {
        let source = r#"
use std::io;

fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let chunks = chunk_source(source, "test.rs", "rust", "function").unwrap();
        assert!(
            chunks.len() >= 2,
            "Expected at least 2 chunks, got {}",
            chunks.len()
        );

        let names: Vec<Option<&str>> = chunks.iter().map(|c| c.name.as_deref()).collect();
        assert!(names.contains(&Some("main")));
        assert!(names.contains(&Some("add")));
    }

    #[test]
    fn test_chunk_ids_are_deterministic() {
        let source = "def hello():\n    pass\n";
        let chunks1 = chunk_source(source, "test.py", "python", "function").unwrap();
        let chunks2 = chunk_source(source, "test.py", "python", "function").unwrap();
        assert_eq!(chunks1[0].id, chunks2[0].id);
    }

    #[test]
    fn test_empty_file_produces_module_chunk() {
        let source = "# just a comment\n";
        let chunks = chunk_source(source, "empty.py", "python", "function").unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::Module);
    }

    #[test]
    fn test_extract_imports_typescript() {
        let source = "import { foo } from './foo';\nimport bar from 'bar';\nconst x = 1;";
        let imports = extract_imports(source, "typescript");
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn test_extract_imports_python() {
        let source = "import os\nfrom pathlib import Path\nx = 1";
        let imports = extract_imports(source, "python");
        assert_eq!(imports.len(), 2);
    }
}
