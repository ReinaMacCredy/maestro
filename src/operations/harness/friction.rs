/// Return true when a user prompt looks like a correction or interruption.
pub fn looks_like_correction(text: &str) -> bool {
    looks_like_correction_with_policy(text, false)
}

/// Return true when a user prompt looks like a correction under the stricter
/// escalation policy, where a correction keyword is required.
pub fn looks_like_correction_requiring_keyword(text: &str) -> bool {
    looks_like_correction_with_policy(text, true)
}

fn looks_like_correction_with_policy(text: &str, require_keyword: bool) -> bool {
    let lower = text.to_ascii_lowercase();
    let trimmed = lower.trim();
    let short_prompt = trimmed.split_whitespace().count() <= 4 && trimmed.len() <= 48;
    let correction_keyword = trimmed.contains("actually")
        || trimmed.contains("wait")
        || trimmed.contains(" no ")
        || trimmed.contains(" no,")
        || trimmed.starts_with("no,")
        || trimmed.starts_with("no ")
        || trimmed == "no";

    if require_keyword {
        correction_keyword && trimmed.len() <= 160
    } else {
        short_prompt || (correction_keyword && trimmed.len() <= 160)
    }
}
