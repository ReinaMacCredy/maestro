use std::fs;
use std::path::{Path, PathBuf};

const PRODUCTION_UNWRAP_ALLOWLIST: &[(&str, usize)] = &[];

#[test]
fn production_sources_do_not_call_unwrap() {
    let mut violations = Vec::new();
    for path in rust_files_under(Path::new("src")) {
        let source = read_source_file(&path);
        for (line_number, line) in production_lines(&source) {
            if line.contains(".unwrap()") && !is_allowlisted(&path, line_number) {
                violations.push(format!("{}:{line_number}: {line}", path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production sources must not call .unwrap(); use Result propagation, \
         an invariant expect, or an infallible rendering pattern instead:\n{}",
        violations.join("\n")
    );
}

fn is_allowlisted(path: &Path, line: usize) -> bool {
    let path = path.to_string_lossy();
    PRODUCTION_UNWRAP_ALLOWLIST
        .iter()
        .any(|(allowed_path, allowed_line)| path == *allowed_path && line == *allowed_line)
}

fn production_lines(source: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut cfg_test_pending = false;
    let mut skipped_brace_depth: Option<usize> = None;
    let mut lexer = BraceLexer::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let starts_in_code = lexer.in_code();

        if let Some(depth) = skipped_brace_depth.as_mut() {
            *depth = lexer.next_depth(*depth, line);
            if *depth == 0 {
                skipped_brace_depth = None;
            }
            continue;
        }

        let trimmed = line.trim_start();
        if starts_in_code && is_cfg_test_attr(trimmed) {
            lexer.next_depth(0, line);
            cfg_test_pending = true;
            continue;
        }

        if cfg_test_pending {
            if starts_in_code && trimmed.starts_with("#[") {
                lexer.next_depth(0, line);
                continue;
            }
            let depth = lexer.next_depth(0, line);
            if depth > 0 {
                skipped_brace_depth = Some(depth);
            }
            cfg_test_pending = false;
            continue;
        }

        lexer.next_depth(0, line);
        lines.push((line_number, line.to_string()));
    }

    lines
}

fn is_cfg_test_attr(trimmed: &str) -> bool {
    trimmed.starts_with("#[cfg(test)]")
        || trimmed.starts_with("#[cfg(any(test")
        || trimmed.starts_with("#[cfg(all(test")
}

/// Counts brace depth while skipping braces inside string literals, char
/// literals, and comments, carrying string/comment state across lines so a
/// multi-line literal cannot silently extend or shorten a skipped region.
struct BraceLexer {
    state: LexState,
}

#[derive(Clone, Copy)]
enum LexState {
    Code,
    BlockComment(usize),
    Str,
    RawStr(usize),
}

impl BraceLexer {
    fn new() -> Self {
        Self {
            state: LexState::Code,
        }
    }

    fn in_code(&self) -> bool {
        matches!(self.state, LexState::Code)
    }

    fn next_depth(&mut self, current: usize, line: &str) -> usize {
        let chars: Vec<char> = line.chars().collect();
        let mut depth = current;
        let mut i = 0;
        while i < chars.len() {
            match self.state {
                LexState::BlockComment(nesting) => {
                    if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        self.state = if nesting == 1 {
                            LexState::Code
                        } else {
                            LexState::BlockComment(nesting - 1)
                        };
                        i += 2;
                    } else if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        self.state = LexState::BlockComment(nesting + 1);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                LexState::Str => {
                    if chars[i] == '\\' {
                        i += 2;
                    } else if chars[i] == '"' {
                        self.state = LexState::Code;
                        i += 1;
                    } else {
                        i += 1;
                    }
                }
                LexState::RawStr(hashes) => {
                    let closes = chars[i] == '"'
                        && chars[i + 1..].iter().take_while(|ch| **ch == '#').count() >= hashes;
                    if closes {
                        self.state = LexState::Code;
                        i += 1 + hashes;
                    } else {
                        i += 1;
                    }
                }
                LexState::Code => match chars[i] {
                    '/' if chars.get(i + 1) == Some(&'/') => break,
                    '/' if chars.get(i + 1) == Some(&'*') => {
                        self.state = LexState::BlockComment(1);
                        i += 2;
                    }
                    '"' => {
                        self.state = LexState::Str;
                        i += 1;
                    }
                    '\'' => i = skip_char_literal(&chars, i),
                    '{' => {
                        depth = depth.saturating_add(1);
                        i += 1;
                    }
                    '}' => {
                        depth = depth.saturating_sub(1);
                        i += 1;
                    }
                    'r' | 'b' | 'c' if !preceded_by_ident_char(&chars, i) => {
                        if let Some((hashes, len)) = raw_string_start(&chars, i) {
                            self.state = LexState::RawStr(hashes);
                            i += len;
                        } else {
                            i += 1;
                        }
                    }
                    _ => i += 1,
                },
            }
        }
        depth
    }
}

fn preceded_by_ident_char(chars: &[char], i: usize) -> bool {
    i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')
}

/// Returns (hash count, prefix length) when `chars[i..]` starts a raw string
/// literal such as `r"`, `r#"`, `br#"`, or `cr#"`.
fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if chars[j] == 'b' || chars[j] == 'c' {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let hashes = chars[j..].iter().take_while(|ch| **ch == '#').count();
    j += hashes;
    if chars.get(j) == Some(&'"') {
        Some((hashes, j + 1 - i))
    } else {
        None
    }
}

/// Returns the index just past a char literal starting at `chars[i] == '\''`,
/// or `i + 1` when the quote opens a lifetime instead.
fn skip_char_literal(chars: &[char], i: usize) -> usize {
    match chars.get(i + 1) {
        Some('\\') => {
            let mut j = i + 2;
            while j < chars.len() && chars[j] != '\'' {
                j += 1;
            }
            j + 1
        }
        Some(_) if chars.get(i + 2) == Some(&'\'') => i + 3,
        _ => i + 1,
    }
}

fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to scan {}: {error}", dir.display()))
    {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to read entry under {}: {error}", dir.display())
        });
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn read_source_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn production_lines_skips_cfg_all_test_regions() {
    let source = "fn keep() {}\n\n#[cfg(all(test, unix))]\nmod tests {\n    fn hidden() {}\n}\n\nfn also_keep() {}\n";
    let lines = production_lines(source);
    let joined = lines
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("fn keep"));
    assert!(joined.contains("fn also_keep"));
    assert!(!joined.contains("fn hidden"));
}

#[test]
fn skipped_region_is_not_ended_by_braces_inside_strings() {
    let source = "#[cfg(test)]\nmod tests {\n    const SNIPPET: &str = \"}\";\n    fn helper() {}\n}\n\nfn production() {}\n";
    let lines = production_lines(source);
    let joined = lines
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("fn production"));
    assert!(!joined.contains("SNIPPET"));
    assert!(!joined.contains("fn helper"));
}

#[test]
fn attr_text_inside_a_multiline_string_does_not_start_a_skip() {
    let source = "const DOC: &str = \"\n#[cfg(test)]\nmod tests {\n\";\nfn production() {}\n";
    let lines = production_lines(source);
    assert!(lines.iter().any(|(_, line)| line.contains("fn production")));
}

#[test]
fn brace_lexer_ignores_braces_in_literals_and_comments() {
    let mut lexer = BraceLexer::new();
    assert_eq!(lexer.next_depth(0, "fn f() {"), 1);
    assert_eq!(lexer.next_depth(1, "    let raw = r#\"} } }\"#;"), 1);
    assert_eq!(lexer.next_depth(1, "    let byte_raw = br#\"}\"#;"), 1);
    assert_eq!(lexer.next_depth(1, "    let close = '}';"), 1);
    assert_eq!(lexer.next_depth(1, "    let escaped = '\\u{7FFF}';"), 1);
    assert_eq!(lexer.next_depth(1, "    // } } }"), 1);
    assert_eq!(
        lexer.next_depth(1, "    /* } */ let lifetime: &'static str = \"}\";"),
        1
    );
    assert_eq!(lexer.next_depth(1, "}"), 0);
}

#[test]
fn brace_lexer_carries_string_state_across_lines() {
    let mut lexer = BraceLexer::new();
    assert_eq!(lexer.next_depth(0, "const X: &str = \"start"), 0);
    assert!(!lexer.in_code());
    assert_eq!(lexer.next_depth(0, "} still inside the string"), 0);
    assert_eq!(lexer.next_depth(0, "end\";"), 0);
    assert!(lexer.in_code());
}

#[test]
fn brace_lexer_carries_block_comment_state_across_lines() {
    let mut lexer = BraceLexer::new();
    assert_eq!(lexer.next_depth(0, "/* outer /* nested } */"), 0);
    assert!(!lexer.in_code());
    assert_eq!(lexer.next_depth(0, "still commented }"), 0);
    assert_eq!(lexer.next_depth(0, "*/ fn after() {"), 1);
    assert!(lexer.in_code());
}
