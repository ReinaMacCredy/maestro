use anyhow::Result;

use crate::foundation::core::time::{
    format_utc_seconds_rfc3339_millis, relative_age_from_unix_seconds,
};

/// Execute `maestro version`: print the embedded version (with a rendered
/// `(released …, … ago)` tail) and the running binary's path.
pub fn run() -> Result<()> {
    println!("maestro {}", version_line(env!("MAESTRO_VERSION")));
    println!("binary: {}", std::env::args().next().unwrap_or_default());
    Ok(())
}

/// Build the version line: `<version> (released <rfc3339>, <age>)`, with the tail
/// rendered from the version's own commit-epoch (no network). Falls back to the bare
/// version for the non-git `<major>.<minor>.<patch>-gunknown` build, where there is no epoch to render.
fn version_line(version: &str) -> String {
    let Some(epoch) = version_commit_epoch(version) else {
        return version.to_string();
    };
    let released = format_utc_seconds_rfc3339_millis(epoch);
    match relative_age_from_unix_seconds(epoch as i64) {
        Some(age) => format!("{version} (released {released}, {age})"),
        None => format!("{version} (released {released})"),
    }
}

/// Extract the Unix-epoch-seconds component from `<major>.<minor>.<patch>.<epoch>-g<sha>`. The
/// epoch is always the 4th dot-component, so the parse is independent of the semver prefix.
/// Returns None for the `<major>.<minor>.<patch>-gunknown` fallback (no 4th component) or any
/// version without a numeric epoch.
fn version_commit_epoch(version: &str) -> Option<u64> {
    let epoch = version.split('.').nth(3)?;
    let epoch = epoch.split('-').next()?.parse::<u64>().ok()?;
    (epoch != 0).then_some(epoch)
}

#[cfg(test)]
mod tests {
    use super::{version_commit_epoch, version_line};

    #[test]
    fn parses_the_commit_epoch_from_an_amp_style_version() {
        assert_eq!(
            version_commit_epoch("0.107.0.1779772576-g751b94"),
            Some(1779772576)
        );
    }

    #[test]
    fn parses_the_commit_epoch_regardless_of_the_semver_prefix() {
        // The runtime scheme carries Cargo.toml's full semver (e.g. `0.107.0.`); the epoch
        // is always the 4th dot-component, so the prefix must not shift the parse.
        assert_eq!(
            version_commit_epoch("9.9.9.1780323053-g0465a733"),
            Some(1780323053)
        );
    }

    #[test]
    fn renders_the_released_tail_with_full_iso_and_age() {
        let line = version_line("0.107.0.1779772576-g751b94");
        assert!(
            line.starts_with("0.107.0.1779772576-g751b94 (released 2026-05-26T05:16:16.000Z, "),
            "unexpected version line: {line}"
        );
        assert!(line.ends_with(" ago)"), "unexpected version line: {line}");
    }

    #[test]
    fn fallback_version_renders_without_a_released_tail() {
        // The non-git `<major>.<minor>.<patch>-gunknown` build has no 4th component -> no
        // `(released …)` tail, so it never prints a bogus 1970-01-01 epoch.
        assert_eq!(version_commit_epoch("0.107.0-gunknown"), None);
        assert_eq!(version_line("0.107.0-gunknown"), "0.107.0-gunknown");
    }
}
