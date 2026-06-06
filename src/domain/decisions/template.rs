use crate::foundation::core::slug::slugify_ascii;

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

/// Render a complete decision record from CLI-provided sections.
pub fn decision_markdown_with_sections(
    number: u32,
    title: &str,
    context: Option<&str>,
    decision: Option<&str>,
    alternatives: &[String],
    consequences: &[String],
    feature: Option<&str>,
) -> String {
    let id = format!("decision-{number:03}");
    let mut out = format!("# {id}: {title}\n\n## Status\nAccepted\n\n## Context\n");
    push_optional_block(&mut out, context);
    out.push_str("\n## Decision\n");
    push_optional_block(&mut out, decision);
    out.push_str("\n## Alternatives considered\n");
    push_list_or_blank(&mut out, alternatives);
    out.push_str("\n## Consequences\n");
    push_list_or_blank(&mut out, consequences);
    out.push_str("\n## Linked tasks\n");
    if let Some(feature) = feature.map(str::trim).filter(|value| !value.is_empty()) {
        out.push_str(&format!("- feature: {feature}\n"));
    }
    out.push('\n');
    out
}

fn push_optional_block(out: &mut String, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        out.push_str(value);
        out.push('\n');
    }
}

fn push_list_or_blank(out: &mut String, values: &[String]) {
    for value in values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        out.push_str(&format!("- {value}\n"));
    }
}
