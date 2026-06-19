//! The logical session token for the running process, resolved from the agent
//! session environment. One source of truth so every layer -- the CLI run-id,
//! the active-session union, and the heavy-run gate lock -- buckets a session
//! under the same id. Foundation-level (no domain knowledge): just env in,
//! token out.

use std::env;

/// The first non-empty agent session id from the environment, trimmed; or a
/// `cli-<date>` fallback when none is set so every CLI-path event in one calendar
/// day at least shares a bucket.
pub fn session_token() -> String {
    for key in [
        "MAESTRO_SESSION_ID",
        "MAESTRO_RUN_ID",
        // Codex CLI's real per-session id (the verified var is CODEX_THREAD_ID,
        // not the never-set CODEX_SESSION_ID it replaces); ordered ahead of the
        // CLAUDE keys so a Codex run buckets under its thread.
        "CODEX_THREAD_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
        // Claude Code's real per-session id; without it every CLI-path event in a
        // Claude session collapses into one cli-<date> bucket (D9).
        "CLAUDE_CODE_SESSION_ID",
    ] {
        if let Ok(value) = env::var(key)
            && !value.trim().is_empty()
        {
            // Trimmed: the raw value becomes a claim/assignee token, and
            // stray whitespace would break later equality lookups.
            return value.trim().to_string();
        }
    }
    let date = crate::foundation::core::time::utc_now_timestamp()
        .split_once('T')
        .map(|(date, _)| date.to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());
    format!("cli-{date}")
}
