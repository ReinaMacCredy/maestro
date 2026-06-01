use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=embedded/skills");
    emit_version();
}

/// Stamp the binary with an Amp-style version `0.0.<commit-epoch>-g<short-sha>`.
///
/// Precedence: an explicit `MAESTRO_VERSION` (the release workflow computes it once
/// and passes it so the build and the git tag agree) > a value derived from git >
/// a static `0.0.0-gunknown` for non-git builds (e.g. a source tarball).
fn emit_version() {
    println!("cargo:rerun-if-env-changed=MAESTRO_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Some(ref_path) = git_head_ref_path() {
        println!("cargo:rerun-if-changed={ref_path}");
    }

    let version = std::env::var("MAESTRO_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(version_from_git);
    println!("cargo:rustc-env=MAESTRO_VERSION={version}");
}

/// Derive `0.0.<commit-epoch>-g<short-sha>` from git, matching Amp's version format.
/// Falls back to `0.0.0-gunknown` when git is unavailable.
fn version_from_git() -> String {
    match (
        git(&["log", "-1", "--format=%ct"]),
        git(&["rev-parse", "--short", "HEAD"]),
    ) {
        (Some(epoch), Some(sha)) => format!("0.0.{epoch}-g{sha}"),
        _ => "0.0.0-gunknown".to_string(),
    }
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Resolve `.git/HEAD`'s symbolic ref (e.g. `refs/heads/main`) so the build re-runs
/// when a new commit lands on the current branch.
fn git_head_ref_path() -> Option<String> {
    let head = std::fs::read_to_string(".git/HEAD").ok()?;
    let reference = head.strip_prefix("ref:")?.trim();
    Some(format!(".git/{reference}"))
}
