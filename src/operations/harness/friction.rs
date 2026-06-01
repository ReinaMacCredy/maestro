/// Return true when a user prompt looks like a correction or interruption.
pub fn looks_like_correction(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let trimmed = lower.trim();
    let short_prompt = trimmed.split_whitespace().count() <= 4 && trimmed.len() <= 48;
    let correction_keyword = trimmed.contains("actually")
        || trimmed.contains("wait")
        || trimmed.contains(" no ")
        || trimmed.starts_with("no ")
        || trimmed == "no";

    short_prompt || (correction_keyword && trimmed.len() <= 160)
}
