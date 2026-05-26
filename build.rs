use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=MAESTRO_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=MAESTRO_BUILD_COMMIT");
    println!("cargo:rerun-if-changed=VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let version = std::env::var("MAESTRO_BUILD_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(build_version);
    println!("cargo:rustc-env=MAESTRO_BUILD_VERSION={version}");
}

fn build_version() -> String {
    let number = include_str!("VERSION").trim();
    assert!(
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()),
        "VERSION must contain only the numeric release version"
    );
    let short_hash = std::env::var("MAESTRO_BUILD_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(git_short_hash);
    format!("0.0.{number}-g{}", short_hash.trim_start_matches('g'))
}

fn git_short_hash() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .expect("failed to run git for Maestro build version");
    assert!(
        output.status.success(),
        "failed to read git commit for Maestro build version"
    );
    let value = String::from_utf8(output.stdout)
        .expect("git commit for Maestro build version was not UTF-8");
    let value = value.trim();
    assert!(
        !value.is_empty(),
        "git commit for Maestro build version was empty"
    );
    value.to_string()
}
