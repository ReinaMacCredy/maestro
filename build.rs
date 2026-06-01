use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=embedded/skills");
    emit_version();
}

/// Stamp the binary with an Amp-style version `<major>.<minor>.<patch>.<commit-epoch>-g<short-sha>`.
///
/// `<major>.<minor>.<patch>` is Cargo.toml's full semver (e.g. `0.107.0`), so bumping the
/// crate version steps the line; the commit-epoch follows as a 4th dotted component.
///
/// Precedence: an explicit `MAESTRO_VERSION` (the release workflow computes it once
/// and passes it so the build and the git tag agree) > a value derived from git >
/// a static `<major>.<minor>.<patch>-gunknown` for non-git builds (e.g. a source tarball).
fn emit_version() {
    println!("cargo:rerun-if-env-changed=MAESTRO_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Some(ref_path) = git_head_ref_path() {
        println!("cargo:rerun-if-changed={ref_path}");
    }

    let version = env_nonempty("MAESTRO_VERSION").unwrap_or_else(version_from_git);
    println!("cargo:rustc-env=MAESTRO_VERSION={version}");
}

/// Derive `<major>.<minor>.<patch>.<commit-epoch>-g<short-sha>` from git, matching Amp's
/// format. Falls back to `<major>.<minor>.<patch>-gunknown` when git is unavailable.
fn version_from_git() -> String {
    let prefix = version_prefix();
    match (
        git(&["log", "-1", "--format=%ct"]),
        git(&["rev-parse", "--short", "HEAD"]),
    ) {
        (Some(epoch), Some(sha)) => format!("{prefix}.{epoch}-g{sha}"),
        _ => format!("{prefix}-gunknown"),
    }
}

/// Cargo.toml's full semver, e.g. crate `0.107.0`. Cargo sets `CARGO_PKG_VERSION` for
/// build scripts; defaults to `0.0.0` if absent.
fn version_prefix() -> String {
    env_nonempty("CARGO_PKG_VERSION").unwrap_or_else(|| "0.0.0".to_string())
}

/// Read an env var, treating a blank or whitespace-only value as absent.
fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
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
