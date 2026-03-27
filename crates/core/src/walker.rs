use crate::{Config, detect_language};
use anyhow::Result;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub language: String,
    pub size_bytes: u64,
    pub line_count: usize,
}

/// Walk a repository respecting .gitignore and config include/exclude globs.
/// Returns a list of files with metadata, filtered to supported languages.
pub fn walk_repo(repo_path: &str, config: &Config) -> Result<Vec<FileEntry>> {
    let repo = Path::new(repo_path);
    if !repo.is_dir() {
        anyhow::bail!(
            "Repository path does not exist or is not a directory: {}",
            repo_path
        );
    }

    let mut builder = WalkBuilder::new(repo);
    builder
        .hidden(true) // skip hidden files by default
        .git_ignore(true) // respect .gitignore
        .git_global(true)
        .git_exclude(true);

    // Add custom ignore globs from config exclude patterns
    for pattern in &config.exclude {
        let mut override_builder = ignore::overrides::OverrideBuilder::new(repo);
        override_builder.add(&format!("!{}", pattern))?;
        if let Ok(overrides) = override_builder.build() {
            builder.overrides(overrides);
        }
    }

    let walker = builder.build();
    let mut files = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: skipping entry: {}", e);
                continue;
            }
        };

        // Skip directories
        if entry.file_type().is_none_or(|ft| ft.is_dir()) {
            continue;
        }

        let path = entry.path();

        // Detect language — skip unsupported extensions
        let language = match detect_language(path) {
            Some(lang) => lang,
            None => continue,
        };

        // Check include patterns if specified
        if !config.include.is_empty() && !matches_any_glob(path, repo, &config.include) {
            continue;
        }

        // Check exclude patterns
        if matches_any_glob(path, repo, &config.exclude) {
            continue;
        }

        // Read file metadata
        let metadata = std::fs::metadata(path)?;
        let size_bytes = metadata.len();

        let content = std::fs::read_to_string(path).unwrap_or_default();
        let line_count = content.lines().count();

        let relative_path = path
            .strip_prefix(repo)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        files.push(FileEntry {
            path: relative_path,
            language,
            size_bytes,
            line_count,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Check if a path matches any of the given glob patterns.
fn matches_any_glob(path: &Path, base: &Path, patterns: &[String]) -> bool {
    let relative = path.strip_prefix(base).unwrap_or(path);
    let rel_str = relative.to_string_lossy();

    for pattern in patterns {
        if let Ok(glob) = glob::Pattern::new(pattern)
            && glob.matches(&rel_str)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        fs::write(
            src.join("main.ts"),
            "function hello() {\n  console.log('hello');\n}\n",
        )
        .unwrap();

        fs::write(
            src.join("utils.py"),
            "def greet(name):\n    return f'Hello, {name}'\n",
        )
        .unwrap();

        fs::write(src.join("readme.md"), "# Test repo\n").unwrap();

        // Init a git repo so .gitignore is respected by the ignore crate
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Create a .gitignore
        fs::write(dir.path().join(".gitignore"), "node_modules/\n*.log\n").unwrap();

        // Create node_modules (should be ignored)
        let nm = dir.path().join("node_modules");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("package.js"), "module.exports = {};").unwrap();

        dir
    }

    #[test]
    fn test_walk_repo_finds_supported_files() {
        let repo = create_test_repo();
        let config = Config::default();
        let files = walk_repo(repo.path().to_str().unwrap(), &config).unwrap();

        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"src/main.ts"));
        assert!(paths.contains(&"src/utils.py"));
        // readme.md is not a supported language
        assert!(!paths.iter().any(|p| p.contains("readme")));
        // node_modules should be gitignored
        assert!(!paths.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn test_walk_repo_include_filter() {
        let repo = create_test_repo();
        let config = Config {
            include: vec!["src/**/*.ts".to_string()],
            ..Config::default()
        };
        let files = walk_repo(repo.path().to_str().unwrap(), &config).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].language, "typescript");
    }

    #[test]
    fn test_walk_repo_exclude_filter() {
        let repo = create_test_repo();
        let config = Config {
            exclude: vec!["**/*.py".to_string()],
            ..Config::default()
        };
        let files = walk_repo(repo.path().to_str().unwrap(), &config).unwrap();

        assert!(!files.iter().any(|f| f.language == "python"));
    }

    #[test]
    fn test_walk_repo_file_metadata() {
        let repo = create_test_repo();
        let config = Config::default();
        let files = walk_repo(repo.path().to_str().unwrap(), &config).unwrap();

        for file in &files {
            assert!(file.size_bytes > 0);
            assert!(file.line_count > 0);
            assert!(!file.language.is_empty());
        }
    }
}
