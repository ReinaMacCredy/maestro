//! The logical session token for the running process, resolved from the agent
//! session environment. One source of truth so every layer -- the CLI run-id,
//! the active-session union, and the heavy-run gate lock -- buckets a session
//! under the same id. Foundation-level (no domain knowledge): just env in,
//! token out.

use std::env;

/// The ordered env keys that carry a real per-session id, agent-runtime ids
/// first. The single source the run-id, the active-session union, the gate lock,
/// and the card-claim identity all scan -- so a session buckets under one id
/// everywhere (D9). `CODEX_THREAD_ID` is Codex CLI's verified per-session var
/// (the never-set `CODEX_SESSION_ID` it replaced is intentionally absent), and
/// `CLAUDE_CODE_SESSION_ID` is Claude Code's; both must stay here or a real
/// runtime falls through to a fallback and self-locks (bug-claim-...-2ce0).
pub const SESSION_ENV_KEYS: &[&str] = &[
    "MAESTRO_SESSION_ID",
    "MAESTRO_RUN_ID",
    "CODEX_THREAD_ID",
    "CLAUDE_SESSION_ID",
    "CLAUDECODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ID",
];

const AGENT_RUNTIME_ENV_KEY: &str = "MAESTRO_AGENT";
const CLAUDE_RUNTIME_ENV_KEYS: &[&str] = &["CLAUDECODE", "CLAUDE_CODE"];
const CODEX_RUNTIME_ENV_KEYS: &[&str] = &["CODEX_CLI", "CODEX_SANDBOX"];

/// First non-empty, trimmed value among `keys`, resolved through `lookup`. Pure:
/// the caller supplies the environment, so the key list is testable without
/// mutating the process-global env.
fn first_present(keys: &[&str], lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    keys.iter().find_map(|key| {
        lookup(key)
            // Trimmed: the value becomes a claim/assignee token, and stray
            // whitespace would break later equality lookups.
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// The agent session id from the environment, or `None` when none is set.
pub fn session_id_from_env() -> Option<String> {
    first_present(SESSION_ENV_KEYS, |key| env::var(key).ok())
}

pub fn known_agent_runtime(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("claude") {
        Some("claude")
    } else if value.eq_ignore_ascii_case("codex") {
        Some("codex")
    } else {
        None
    }
}

fn any_present(keys: &[&str], lookup: &impl Fn(&str) -> Option<String>) -> bool {
    keys.iter()
        .any(|key| lookup(key).is_some_and(|value| !value.trim().is_empty()))
}

fn agent_runtime_from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    if let Some(agent) = lookup(AGENT_RUNTIME_ENV_KEY)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return known_agent_runtime(&agent);
    }
    if any_present(CLAUDE_RUNTIME_ENV_KEYS, &lookup) {
        return Some("claude");
    }
    if any_present(CODEX_RUNTIME_ENV_KEYS, &lookup) {
        return Some("codex");
    }
    None
}

/// The known agent runtime for this process, if Claude or Codex can be resolved.
pub fn agent_runtime_from_env() -> Option<&'static str> {
    agent_runtime_from_lookup(|key| env::var(key).ok())
}

/// The first non-empty agent session id from the environment, trimmed; or a
/// `cli-<date>` fallback when none is set so every CLI-path event in one calendar
/// day at least shares a bucket.
pub fn session_token() -> String {
    session_id_from_env().unwrap_or_else(|| {
        let date = crate::foundation::core::time::utc_now_timestamp()
            .split_once('T')
            .map(|(date, _)| date.to_string())
            .unwrap_or_else(|| "1970-01-01".to_string());
        format!("cli-{date}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_present_prefers_the_earliest_present_key_and_trims() {
        let got = first_present(SESSION_ENV_KEYS, |key| {
            // Two keys set; the earlier one in the list wins.
            match key {
                "MAESTRO_RUN_ID" => Some("  run-7  ".to_string()),
                "CLAUDE_CODE_SESSION_ID" => Some("claude-9".to_string()),
                _ => None,
            }
        });
        assert_eq!(got, Some("run-7".to_string()));
    }

    #[test]
    fn first_present_scans_the_real_runtime_session_keys() {
        // The regression bug-claim-...-2ce0 was a missing key: the real Codex and
        // Claude Code per-session vars must each resolve on their own.
        for key in ["CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID"] {
            let got = first_present(SESSION_ENV_KEYS, |scanned| {
                (scanned == key).then(|| "id-x".to_string())
            });
            assert_eq!(
                got,
                Some("id-x".to_string()),
                "{key} must be in the shared session env scan"
            );
        }
    }

    #[test]
    fn first_present_is_none_when_nothing_is_set_or_only_blank() {
        assert_eq!(first_present(SESSION_ENV_KEYS, |_| None), None);
        assert_eq!(
            first_present(SESSION_ENV_KEYS, |_| Some("   ".to_string())),
            None
        );
    }

    fn agent_runtime_from_pairs(pairs: &[(&str, &str)]) -> Option<&'static str> {
        agent_runtime_from_lookup(|key| {
            pairs
                .iter()
                .find_map(|(name, value)| (*name == key).then(|| (*value).to_string()))
        })
    }

    #[test]
    fn agent_runtime_prefers_explicit_maestro_agent() {
        let got = agent_runtime_from_pairs(&[
            ("MAESTRO_AGENT", "  Claude  "),
            ("CODEX_CLI", "1"),
            ("CODEX_SANDBOX", "1"),
        ]);
        assert_eq!(got, Some("claude"));
    }

    #[test]
    fn agent_runtime_uses_known_runtime_env_as_backup() {
        assert_eq!(
            agent_runtime_from_pairs(&[("CLAUDE_CODE", "1")]),
            Some("claude")
        );
        assert_eq!(
            agent_runtime_from_pairs(&[("CODEX_SANDBOX", "1")]),
            Some("codex")
        );
    }

    #[test]
    fn agent_runtime_omits_unknown_or_blank_values() {
        assert_eq!(
            agent_runtime_from_pairs(&[("MAESTRO_AGENT", "gemini"), ("CODEX_SANDBOX", "1")]),
            None
        );
        assert_eq!(
            agent_runtime_from_pairs(&[("MAESTRO_AGENT", "   "), ("CODEX_SANDBOX", "1")]),
            Some("codex")
        );
        assert_eq!(agent_runtime_from_pairs(&[("CLAUDECODE", "   ")]), None);
    }
}
