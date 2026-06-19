//! Lean mode: the per-session strictness dial for the reach-ladder.
//!
//! The reach-ladder (climb stdlib -> platform -> installed dep -> one-liner ->
//! minimal new code before writing) is always-on guidance in the harness. Lean
//! mode tunes how hard the agent enforces it for the current session: `ultra`
//! rejects non-minimal code, `full` applies the cheaper version, `lite`
//! suggests, `off` suppresses the proactive climb step. The mode is keyed to the
//! session's run dir, so concurrent sessions are independent and a new session
//! starts from the `MAESTRO_LEAN` default rather than inheriting a prior set --
//! no stale-state sweep is needed because a new session is a new run dir.

use std::fmt;
use std::path::PathBuf;

use anyhow::Result;

use crate::domain::run::run_dir_name;
use crate::foundation::core::fs::{ensure_parent_dir, read_to_string_if_exists};
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;

/// How strictly the reach-ladder is enforced for a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeanMode {
    /// Suggest the cheaper version, do not apply.
    Lite,
    /// Apply the cheaper version (the default).
    Full,
    /// Reject non-minimal code a lower rung already covers.
    Ultra,
    /// Suppress the proactive climb step for the session.
    Off,
}

impl LeanMode {
    /// The mode applied when nothing is stored and no `MAESTRO_LEAN` default set.
    pub const DEFAULT: LeanMode = LeanMode::Full;

    /// The canonical token for this mode, as stored and printed.
    pub fn as_str(self) -> &'static str {
        match self {
            LeanMode::Lite => "lite",
            LeanMode::Full => "full",
            LeanMode::Ultra => "ultra",
            LeanMode::Off => "off",
        }
    }

    /// Parse a mode token case-insensitively; `None` for anything else.
    pub fn parse(token: &str) -> Option<LeanMode> {
        match token.trim().to_ascii_lowercase().as_str() {
            "lite" => Some(LeanMode::Lite),
            "full" => Some(LeanMode::Full),
            "ultra" => Some(LeanMode::Ultra),
            "off" => Some(LeanMode::Off),
            _ => None,
        }
    }
}

impl fmt::Display for LeanMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The stored-mode file for `session_id`, inside that session's run dir. The
/// session id is percent-encoded into one safe component, so it can never escape
/// `.maestro/runs/`.
fn mode_path(paths: &MaestroPaths, session_id: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/runs/{}/lean_mode", run_dir_name(session_id));
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}

/// The mode stored for this session, or `None` when nothing is stored (or the
/// stored token is unreadable or unrecognized -- best-effort, never an error).
pub fn read_mode(paths: &MaestroPaths, session_id: &str) -> Option<LeanMode> {
    let path = mode_path(paths, session_id).ok()?;
    let raw = read_to_string_if_exists(&path).ok().flatten()?;
    LeanMode::parse(&raw)
}

/// Store `mode` for this session, creating the run dir if absent.
pub fn write_mode(paths: &MaestroPaths, session_id: &str, mode: LeanMode) -> Result<()> {
    let path = mode_path(paths, session_id)?;
    ensure_parent_dir(&path)?;
    write_string_atomic(&path, mode.as_str())
}

/// The effective mode for this session: the stored mode, else the `MAESTRO_LEAN`
/// default if it parses, else [`LeanMode::DEFAULT`].
pub fn resolve_mode(paths: &MaestroPaths, session_id: &str, env_default: Option<&str>) -> LeanMode {
    read_mode(paths, session_id)
        .or_else(|| env_default.and_then(LeanMode::parse))
        .unwrap_or(LeanMode::DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn temp_paths(prefix: &str) -> (TestTempDir, MaestroPaths) {
        let dir = TestTempDir::new(prefix);
        let paths = MaestroPaths::new(dir.path());
        (dir, paths)
    }

    #[test]
    fn parse_round_trips_every_mode() {
        for mode in [LeanMode::Lite, LeanMode::Full, LeanMode::Ultra, LeanMode::Off] {
            assert_eq!(LeanMode::parse(mode.as_str()), Some(mode));
        }
    }

    #[test]
    fn parse_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(LeanMode::parse("ULTRA"), Some(LeanMode::Ultra));
        assert_eq!(LeanMode::parse(" full "), Some(LeanMode::Full));
        assert_eq!(LeanMode::parse("aggressive"), None);
    }

    #[test]
    fn resolve_defaults_to_full_when_nothing_stored_and_no_env() {
        let (_dir, paths) = temp_paths("maestro-lean-default");
        assert_eq!(resolve_mode(&paths, "sess-1", None), LeanMode::Full);
    }

    #[test]
    fn write_then_read_round_trips_for_a_session() {
        let (_dir, paths) = temp_paths("maestro-lean-roundtrip");
        write_mode(&paths, "sess-1", LeanMode::Ultra).unwrap();
        assert_eq!(read_mode(&paths, "sess-1"), Some(LeanMode::Ultra));
        assert_eq!(resolve_mode(&paths, "sess-1", None), LeanMode::Ultra);
    }

    #[test]
    fn env_default_applies_only_when_nothing_is_stored() {
        let (_dir, paths) = temp_paths("maestro-lean-env");
        assert_eq!(resolve_mode(&paths, "sess-1", Some("lite")), LeanMode::Lite);
        write_mode(&paths, "sess-1", LeanMode::Ultra).unwrap();
        assert_eq!(
            resolve_mode(&paths, "sess-1", Some("lite")),
            LeanMode::Ultra,
            "a stored mode wins over the env default"
        );
    }

    #[test]
    fn mode_is_per_session_isolated() {
        let (_dir, paths) = temp_paths("maestro-lean-isolation");
        write_mode(&paths, "sess-A", LeanMode::Ultra).unwrap();
        assert_eq!(read_mode(&paths, "sess-B"), None, "B sees no mode A set");
        assert_eq!(resolve_mode(&paths, "sess-B", None), LeanMode::Full);
        assert_eq!(read_mode(&paths, "sess-A"), Some(LeanMode::Ultra));
    }
}
