//! Enforces the module layout of spec section 4: file sizes, layer direction and file purpose.
// Helpers outside `#[test]` fns are not covered by clippy.toml's allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

const PRODUCTION_LINE_CAP: usize = 250;
const TEST_LINE_CAP: usize = 400;

struct Source {
    /// Path under `src/`, with `/` separators.
    path: String,
    text: String,
}

impl Source {
    fn is_test(&self) -> bool {
        self.path.ends_with("/tests.rs") || self.path.contains("/tests/")
    }

    fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The `use` statements of the file, each expanded to its full paths.
    fn imports(&self) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        let mut lines = self.text.lines();
        while let Some(line) = lines.next() {
            let Some(mut statement) = strip_visibility(line.trim())
                .strip_prefix("use ")
                .map(str::to_owned)
            else {
                continue;
            };
            while !statement.contains(';') {
                match lines.next() {
                    Some(next) => {
                        statement.push(' ');
                        statement.push_str(next.trim());
                    }
                    None => break,
                }
            }
            let tree = statement.split(';').next().unwrap_or_default();
            let tokens = tokenize(tree);
            expand(&tokens, &mut 0, &[], &mut paths);
        }
        paths
    }

    /// True when a string literal on a non-comment line contains `needle`.
    fn has_literal_containing(&self, needle: &str) -> bool {
        let mut is_inside = false;
        for line in self.text.lines() {
            let mut chars = line.chars().peekable();
            let mut literal = String::new();
            while let Some(c) = chars.next() {
                if is_inside {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' => {
                            is_inside = false;
                            if literal.contains(needle) {
                                return true;
                            }
                            literal.clear();
                        }
                        _ => literal.push(c),
                    }
                } else if c == '"' {
                    is_inside = true;
                } else if c == '/' && chars.peek() == Some(&'/') {
                    break;
                }
            }
            if is_inside && literal.contains(needle) {
                return true;
            }
        }
        false
    }
}

fn strip_visibility(line: &str) -> &str {
    let line = line.strip_prefix("pub").map_or(line, |rest| {
        let rest = rest.trim_start();
        if rest.starts_with('(') {
            rest.split_once(')')
                .map_or(rest, |(_, after)| after)
                .trim_start()
        } else {
            rest
        }
    });
    line.trim_start()
}

fn tokenize(tree: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, tokens: &mut Vec<String>| {
        if !word.is_empty() {
            tokens.push(std::mem::take(word));
        }
    };
    for c in tree.chars() {
        match c {
            ':' | '{' | '}' | ',' | '*' => {
                flush(&mut word, &mut tokens);
                if c != ':' || tokens.last().is_none_or(|last| last != ":") {
                    tokens.push(c.to_string());
                } else {
                    tokens.pop();
                    tokens.push("::".to_owned());
                }
            }
            c if c.is_whitespace() => flush(&mut word, &mut tokens),
            c => word.push(c),
        }
    }
    flush(&mut word, &mut tokens);
    tokens
}

/// Expands one use tree, such as `crate::{a::b, c}`, into its full paths.
fn expand(tokens: &[String], pos: &mut usize, prefix: &[String], out: &mut Vec<Vec<String>>) {
    let mut path = prefix.to_vec();
    while let Some(token) = tokens.get(*pos) {
        match token.as_str() {
            "::" => *pos += 1,
            "as" => *pos += 2,
            "{" => {
                *pos += 1;
                while *pos < tokens.len() && tokens[*pos] != "}" {
                    expand(tokens, pos, &path, out);
                    if tokens.get(*pos).is_some_and(|t| t == ",") {
                        *pos += 1;
                    }
                }
                *pos += 1;
                return;
            }
            "," | "}" => break,
            word => {
                path.push(word.to_owned());
                *pos += 1;
            }
        }
    }
    if path.len() > prefix.len() {
        out.push(path);
    }
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<Source>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, root, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let relative = path.strip_prefix(root).unwrap();
            out.push(Source {
                path: relative.to_string_lossy().replace('\\', "/"),
                text: fs::read_to_string(&path).unwrap(),
            });
        }
    }
}

fn sources() -> Vec<Source> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    out
}

fn assert_clean(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} architecture violation(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Files under `folder` that import a `crate::<layer>` in `banned`.
fn reaches(files: &[Source], folder: &str, banned: &[&str]) -> Vec<String> {
    let mut failures = Vec::new();
    for file in files.iter().filter(|f| f.path.starts_with(folder)) {
        for import in file.imports() {
            if import.first().is_some_and(|root| root == "crate")
                && let Some(layer) = import
                    .get(1)
                    .filter(|layer| banned.contains(&layer.as_str()))
            {
                failures.push(format!(
                    "{}: imports crate::{layer}, which {folder} must not reach (layers import only downward)",
                    file.path
                ));
            }
        }
    }
    failures
}

#[test]
fn production_files_stay_under_250_lines() {
    let failures = sources()
        .iter()
        .filter(|f| !f.is_test() && f.text.lines().count() > PRODUCTION_LINE_CAP)
        .map(|f| {
            format!(
                "{}: {} lines, over the {PRODUCTION_LINE_CAP}-line cap; split it by responsibility",
                f.path,
                f.text.lines().count()
            )
        })
        .collect();
    assert_clean(failures);
}

#[test]
fn test_files_stay_under_400_lines() {
    let failures = sources()
        .iter()
        .filter(|f| f.is_test() && f.text.lines().count() > TEST_LINE_CAP)
        .map(|f| {
            format!(
                "{}: {} lines, over the {TEST_LINE_CAP}-line test cap",
                f.path,
                f.text.lines().count()
            )
        })
        .collect();
    assert_clean(failures);
}

#[test]
fn layers_import_only_downward() {
    let files = sources();
    let mut failures = reaches(
        &files,
        "shared/",
        &["relation", "runtime", "features", "http"],
    );
    failures.extend(reaches(
        &files,
        "relation/",
        &["runtime", "features", "http"],
    ));
    for file in files
        .iter()
        .filter(|f| f.path.starts_with("features/") && !f.is_test())
    {
        if matches!(file.file_name(), "routes.rs" | "model.rs") {
            continue;
        }
        for import in file.imports() {
            let uses_http = import.first().is_some_and(|root| root == "axum")
                || import.starts_with(&["crate".to_owned(), "http".to_owned()]);
            if uses_http {
                failures.push(format!(
                    "{}: queries and services must not use axum or crate::http",
                    file.path
                ));
            }
        }
    }
    assert_clean(failures);
}

#[test]
fn shell_layers_cannot_reach_features() {
    assert_clean(reaches(&sources(), "runtime/", &["features", "http"]));
}

#[test]
fn features_do_not_import_each_other() {
    let mut failures = Vec::new();
    for file in sources().iter().filter(|f| f.path.starts_with("features/")) {
        let Some(own) = file.path.split('/').nth(1) else {
            continue;
        };
        for import in file.imports() {
            if let [root, layer, other, ..] = import.as_slice()
                && root == "crate"
                && layer == "features"
                && other != own
            {
                failures.push(format!(
                    "{}: imports crate::features::{other}; a feature never imports another feature",
                    file.path
                ));
            }
        }
    }
    assert_clean(failures);
}

#[test]
fn routes_hold_no_sql() {
    let failures = sources()
        .iter()
        .filter(|f| f.file_name() == "routes.rs" && f.has_literal_containing("SELECT "))
        .map(|f| {
            format!(
                "{}: a routes.rs holds a SELECT literal; SQL belongs in queries.rs",
                f.path
            )
        })
        .collect();
    assert_clean(failures);
}

#[test]
fn every_file_states_its_purpose() {
    let failures = sources()
        .iter()
        .filter(|f| !f.text.starts_with("//!"))
        .map(|f| format!("{}: the first line must be a one-line //! purpose", f.path))
        .collect();
    assert_clean(failures);
}

#[test]
fn import_scan_expands_grouped_and_multiline_use_statements() {
    let source = Source {
        path: "relation/example.rs".to_owned(),
        text: "//! Example.\n\nuse crate::{\n    shared::ids,\n    runtime::{engine::Engine, state},\n};\npub(crate) use axum::Router as Web;\n"
            .to_owned(),
    };

    let mut imports: Vec<String> = source
        .imports()
        .iter()
        .map(|path| path.join("::"))
        .collect();
    imports.sort();

    assert_eq!(
        imports,
        [
            "axum::Router",
            "crate::runtime::engine::Engine",
            "crate::runtime::state",
            "crate::shared::ids"
        ]
    );
}
