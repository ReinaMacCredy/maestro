use crate::core::slug::slugify_ascii;

/// Render the canonical decision file name for a sequence number and title.
pub fn decision_file_name(number: u32, title: &str) -> String {
    format!("decision-{number:03}-{}.md", slugify_ascii(title))
}

/// Render the section 7.4 decision markdown template.
pub fn decision_markdown(number: u32, title: &str) -> String {
    let id = format!("decision-{number:03}");
    format!(
        "# {id}: {title}\n\n## Status\nAccepted\n\n## Context\nWhy this decision exists.\n\n## Decision\nWhat we decided.\n\n## Alternatives considered\n\n## Consequences\n\n## Linked tasks\n\n"
    )
}
