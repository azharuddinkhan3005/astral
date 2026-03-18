use anyhow::Result;
use tree_sitter::{Language, Parser};

/// Get the tree-sitter Language for a given language name.
pub fn get_language(lang: &str) -> Result<Language> {
    match lang {
        "typescript" | "tsx" => Ok(tree_sitter_typescript::language_typescript()),
        "javascript" | "jsx" => Ok(tree_sitter_javascript::language()),
        "python" => Ok(tree_sitter_python::language()),
        "java" => Ok(tree_sitter_java::language()),
        "go" => Ok(tree_sitter_go::language()),
        "rust" => Ok(tree_sitter_rust::language()),
        "ruby" => Ok(tree_sitter_ruby::language()),
        "php" => Ok(tree_sitter_php::language_php()),
        _ => anyhow::bail!("Unsupported language: {}", lang),
    }
}

/// Parse source code into a tree-sitter Tree.
pub fn parse_source(source: &str, lang: &str) -> Result<tree_sitter::Tree> {
    let language = get_language(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse source for language: {}", lang))
}

/// Return the tree-sitter node types that represent chunkable constructs for a language.
pub fn chunk_node_types(lang: &str) -> &[&str] {
    match lang {
        "typescript" | "tsx" | "javascript" | "jsx" => &[
            "function_declaration",
            "method_definition",
            "class_declaration",
            "arrow_function",
            "export_statement",
        ],
        "python" => &["function_definition", "class_definition"],
        "java" => &["method_declaration", "class_declaration"],
        "go" => &["function_declaration", "method_declaration"],
        "rust" => &["function_item", "impl_item"],
        "ruby" => &["method", "class"],
        "php" => &["function_definition", "class_declaration"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typescript() {
        let source = "function hello(): void { console.log('hello'); }";
        let tree = parse_source(source, "typescript").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_python() {
        let source = "def hello():\n    print('hello')\n";
        let tree = parse_source(source, "python").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_javascript() {
        let source = "function add(a, b) { return a + b; }";
        let tree = parse_source(source, "javascript").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_java() {
        let source = "class Foo { void bar() { } }";
        let tree = parse_source(source, "java").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_rust() {
        let source = "fn main() { println!(\"hello\"); }";
        let tree = parse_source(source, "rust").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_go() {
        let source = "package main\nfunc main() { }";
        let tree = parse_source(source, "go").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_php() {
        let source = "<?php\nfunction hello() { echo 'hello'; }\n";
        let tree = parse_source(source, "php").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_unsupported_language() {
        assert!(get_language("cobol").is_err());
    }

    #[test]
    fn test_chunk_node_types_not_empty() {
        for lang in &["typescript", "python", "java", "go", "rust", "ruby", "php"] {
            assert!(
                !chunk_node_types(lang).is_empty(),
                "No chunk types for {}",
                lang
            );
        }
    }
}
