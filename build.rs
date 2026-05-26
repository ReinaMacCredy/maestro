use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=MAESTRO_BUILD_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let version = std::env::var("MAESTRO_BUILD_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_build_version)
        .unwrap_or_else(|| "0.1.0".to_string());
    println!("cargo:rustc-env=MAESTRO_BUILD_VERSION={version}");
}

fn git_build_version() -> Option<String> {
    let timestamp = git_output(["log", "-1", "--format=%ct"])?;
    let short_hash = git_output(["rev-parse", "--short=7", "HEAD"])?;
    Some(format!("0.0.{timestamp}-g{short_hash}"))
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
