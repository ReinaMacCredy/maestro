fn main() {
    println!("cargo:rerun-if-env-changed=MAESTRO_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=MAESTRO_BUILD_COMMIT");
    println!("cargo:rerun-if-changed=VERSION");
    println!("cargo:rerun-if-changed=VERSION_COMMIT");
    println!("cargo:rerun-if-changed=resources/skills");

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
        .unwrap_or_else(|| include_str!("VERSION_COMMIT").trim().to_string());
    assert!(
        !short_hash.is_empty()
            && short_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'g'),
        "VERSION_COMMIT must contain only the release commit suffix"
    );
    format!("0.0.{number}-g{}", short_hash.trim_start_matches('g'))
}
