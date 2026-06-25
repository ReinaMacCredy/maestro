mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn maestro_with_env(args: &[&str], cwd: &Path, envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: git should be runnable in integration tests");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn stderr_failure(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn source_repo(name: &str) -> TestTempDir {
    let temp = TestTempDir::new(name);
    let repo = temp.path();
    git(&["init", "-q"], repo);
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir should be creatable");
    fs::create_dir_all(repo.join("src/bin")).expect("src dir should be creatable");
    fs::create_dir_all(repo.join("vendor")).expect("vendor dir should be creatable");
    fs::create_dir_all(repo.join("generated")).expect("generated dir should be creatable");
    fs::write(repo.join(".gitignore"), "ignored.rs\n").expect("gitignore should be writable");
    fs::write(
        repo.join("src/lib.rs"),
        "pub fn HTTPServer() {\n    println!(\"café Token\");\n}\n",
    )
    .expect("rust source should be writable");
    fs::write(
        repo.join("src/app.py"),
        "def runtime_agent():\n    return 'ready'\n",
    )
    .expect("python source should be writable");
    fs::write(
        repo.join("src/bin/main.rs"),
        "fn main() { HTTPServer(); }\n",
    )
    .expect("nested rust source should be writable");
    fs::write(repo.join("vendor/copied.rs"), "fn vendor_secret() {}\n")
        .expect("vendor source should be writable");
    fs::write(
        repo.join("generated/schema.generated.rs"),
        "fn generated_secret() {}\n",
    )
    .expect("generated source should be writable");
    fs::write(repo.join("binary.dat"), b"hello\0world").expect("binary fixture should be writable");
    fs::write(repo.join("ignored.rs"), "fn ignored_secret() {}\n")
        .expect("ignored source should be writable");
    fs::write(repo.join("huge.txt"), "x".repeat(5 * 1024 * 1024 + 1))
        .expect("oversized fixture should be writable");
    fs::write(repo.join("too_many_lines.txt"), "a\n".repeat(80_001))
        .expect("line cap fixture should be writable");
    temp
}

fn outline_repo(name: &str) -> TestTempDir {
    let temp = TestTempDir::new(name);
    let repo = temp.path();
    git(&["init", "-q"], repo);
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir should be creatable");
    fs::create_dir_all(repo.join("src")).expect("src dir should be creatable");
    fs::create_dir_all(repo.join("docs")).expect("docs dir should be creatable");
    fs::write(
        repo.join("src/lib.rs"),
        r#"use std::path::PathBuf;

pub struct TaskRegistry {
    pub entries: Vec<String>,
    secret: usize,
}

impl TaskRegistry {
    pub fn register_task(&mut self, name: String) {
        self.entries.push(name);
    }
}

pub enum TaskState {
    Ready,
    Closed,
}

pub trait TaskStore {
    fn load_task(&self, path: PathBuf);
}

pub mod runtime_index {}
"#,
    )
    .expect("rust outline fixture should be writable");
    fs::write(
        repo.join("src/ui.ts"),
        r#"import { render } from "./render";

export function renderBoard(): string {
  return render("board");
}

export class BoardPanel {
  open(): void {}
}
"#,
    )
    .expect("typescript outline fixture should be writable");
    fs::write(
        repo.join("src/worker.py"),
        r#"import os

class WorkerAgent:
    def run_job(self):
        return os.getcwd()
"#,
    )
    .expect("python outline fixture should be writable");
    fs::write(
        repo.join("docs/architecture.md"),
        "# Runtime Index\n\n## Source Outline\n",
    )
    .expect("markdown outline fixture should be writable");
    temp
}

fn initialized_outline_repo(name: &str) -> TestTempDir {
    let temp = outline_repo(name);
    let repo = temp.path();
    stdout(maestro(&["init", "--yes"], repo), &["init", "--yes"]);
    stdout(
        maestro(&["harness", "set", "--claims-only"], repo),
        &["harness", "set", "--claims-only"],
    );
    temp
}

#[test]
fn index_rebuild_source_reports_indexed_and_skipped_files() {
    let temp = source_repo("grep-source-rebuild");
    let repo = temp.path();

    let out = stdout(
        maestro(&["index", "rebuild", "--source"], repo),
        &["index", "rebuild", "--source"],
    );

    assert!(out.contains("source shard rebuilt"), "{out}");
    assert!(out.contains("files: 4 indexed"), "{out}");
    for reason in [
        "vendor",
        "generated",
        "binary",
        "gitignored",
        "oversized",
        "line_cap",
    ] {
        assert!(out.contains(&format!("skipped {reason}:")), "{out}");
    }
    assert!(repo.join(".maestro/index/search/source.shard").exists());
}

#[test]
fn grep_source_json_supports_regex_file_lang_case_or_and_negation() {
    let temp = source_repo("grep-source-query");
    let repo = temp.path();
    stdout(
        maestro(&["index", "rebuild", "--source"], repo),
        &["index", "rebuild", "--source"],
    );

    let out = stdout(
        maestro(
            &[
                "grep",
                "--json",
                r#"(/runtime_\w+/ or missing) corpus:source file:src/* lang:python"#,
            ],
            repo,
        ),
        &["grep", "--json", "regex source"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["schema"], "maestro.grep.v1");
    assert_eq!(json["ok"], true);
    assert_eq!(json["hits"][0]["corpus"], "source");
    assert_eq!(json["hits"][0]["kind"], "file");
    assert_eq!(json["hits"][0]["path"], "src/app.py");
    assert_eq!(json["hits"][0]["line"], 1);
    assert_eq!(json["hits"][0]["match_spans"][0]["line"], 1);

    let out = stdout(
        maestro(
            &[
                "grep",
                "--json",
                "httpserver corpus:source case:no lang:rust file:src/lib.rs -Token",
            ],
            repo,
        ),
        &["grep", "--json", "case no negated"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["hits"].as_array().unwrap().len(), 0);

    let out = stdout(
        maestro(
            &[
                "grep",
                "--json",
                "httpserver corpus:source case:no lang:rust file:src/lib.rs",
            ],
            repo,
        ),
        &["grep", "--json", "case no"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["path"], "src/lib.rs");
}

#[test]
fn grep_source_reports_malformed_regex() {
    let temp = source_repo("grep-source-bad-regex");
    let repo = temp.path();

    let err = stderr_failure(
        maestro(&["grep", "/[/", "corpus:source"], repo),
        &["grep", "/[/", "corpus:source"],
    );

    assert!(err.contains("parse_error"), "{err}");
    assert!(err.contains("invalid regex"), "{err}");
}

#[test]
fn grep_source_returns_outline_hits_and_type_filters() {
    let temp = outline_repo("grep-source-outline");
    let repo = temp.path();
    stdout(
        maestro(&["index", "rebuild", "--source"], repo),
        &["index", "rebuild", "--source"],
    );

    let rust = stdout(
        maestro(
            &["grep", "--json", "TaskRegistry corpus:source type:struct"],
            repo,
        ),
        &["grep", "--json", "TaskRegistry corpus:source type:struct"],
    );
    let json: Value = serde_json::from_str(&rust).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["corpus"], "source");
    assert_eq!(json["hits"][0]["kind"], "outline");
    assert_eq!(json["hits"][0]["symbol_kind"], "struct");
    assert_eq!(json["hits"][0]["path"], "src/lib.rs");
    assert!(
        json["hits"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("pub struct TaskRegistry")
    );

    let method = stdout(
        maestro(
            &["grep", "--json", "register_task corpus:source type:method"],
            repo,
        ),
        &["grep", "--json", "register_task corpus:source type:method"],
    );
    let json: Value = serde_json::from_str(&method).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["symbol_kind"], "method");
    assert_eq!(json["hits"][0]["parent"], "TaskRegistry");

    let import = stdout(
        maestro(
            &["grep", "--json", "render corpus:source type:import"],
            repo,
        ),
        &["grep", "--json", "render corpus:source type:import"],
    );
    let json: Value = serde_json::from_str(&import).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["symbol_kind"], "import");
    assert_eq!(json["hits"][0]["path"], "src/ui.ts");

    let export = stdout(
        maestro(
            &["grep", "--json", "renderBoard corpus:source type:export"],
            repo,
        ),
        &["grep", "--json", "renderBoard corpus:source type:export"],
    );
    let json: Value = serde_json::from_str(&export).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["symbol_kind"], "export");
    assert_eq!(json["hits"][0]["path"], "src/ui.ts");

    let python = stdout(
        maestro(
            &[
                "grep",
                "--json",
                "WorkerAgent corpus:source type:struct lang:python",
            ],
            repo,
        ),
        &[
            "grep",
            "--json",
            "WorkerAgent corpus:source type:struct lang:python",
        ],
    );
    let json: Value = serde_json::from_str(&python).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["symbol_kind"], "struct");
    assert_eq!(json["hits"][0]["path"], "src/worker.py");

    let markdown = stdout(
        maestro(
            &[
                "grep",
                "--json",
                "Source Outline corpus:source type:module lang:markdown",
            ],
            repo,
        ),
        &[
            "grep",
            "--json",
            "Source Outline corpus:source type:module lang:markdown",
        ],
    );
    let json: Value = serde_json::from_str(&markdown).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["symbol_kind"], "module");
    assert_eq!(json["hits"][0]["path"], "docs/architecture.md");
}

#[test]
fn sym_missing_ctags_is_clear_and_outline_still_works() {
    let temp = outline_repo("grep-source-sym-missing");
    let repo = temp.path();
    stdout(
        maestro_with_env(&["index", "rebuild", "--source"], repo, &[("PATH", "")]),
        &["index", "rebuild", "--source"],
    );

    let err = stderr_failure(
        maestro_with_env(
            &["grep", "sym:TaskRegistry", "corpus:source"],
            repo,
            &[("PATH", "")],
        ),
        &["grep", "sym:TaskRegistry", "corpus:source"],
    );
    assert!(err.contains("ctags_unavailable"), "{err}");
    assert!(err.contains("install universal-ctags"), "{err}");

    let out = stdout(
        maestro_with_env(
            &["grep", "--json", "TaskRegistry corpus:source type:struct"],
            repo,
            &[("PATH", "")],
        ),
        &["grep", "--json", "TaskRegistry corpus:source type:struct"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["kind"], "outline");
}

#[test]
fn doctor_reports_search_outline_and_optional_ctags_health() {
    let temp = initialized_outline_repo("grep-source-doctor");
    let repo = temp.path();
    stdout(
        maestro_with_env(&["index", "rebuild", "--source"], repo, &[("PATH", "")]),
        &["index", "rebuild", "--source"],
    );

    let out = stdout(
        maestro_with_env(&["doctor"], repo, &[("PATH", "")]),
        &["doctor"],
    );

    assert!(out.contains("check search-index: ok"), "{out}");
    assert!(out.contains("check search-outline: ok"), "{out}");
    assert!(
        out.contains("check search-ctags: ok (optional missing"),
        "{out}"
    );
    assert!(out.contains("doctor: ok"), "{out}");
}
