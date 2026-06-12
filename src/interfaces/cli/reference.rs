//! The generated CLI reference (`reference/cli.md`) shipped inside every
//! embedded skill: authoritative command signatures rendered from the clap
//! model so agents read one file instead of probing `--help`. The committed
//! copies are byte-equality-tested against this renderer in
//! `tests/cli_reference_freshness.rs` and excluded from the resources
//! version guard, so a CLI change only requires regeneration.

use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::foundation::core::hash::sha256_hex;
use crate::interfaces::cli::Cli;

/// Format version of the generated reference, stamped in the header.
pub const CLI_REFERENCE_FORMAT_VERSION: &str = "1.0.0";

/// First body line; everything from here on is covered by the header's
/// sha256 self-check stamp.
const BODY_HEADING: &str = "# maestro CLI reference";

/// The one command that rewrites the committed copies.
pub const REGENERATE_COMMAND: &str =
    "cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored";

/// Render the full reference: a version-stamped self-checking header plus one
/// signature bullet per leaf command.
pub fn render_cli_reference() -> String {
    let mut cmd = Cli::command();
    cmd.build();

    let mut body = String::new();
    body.push_str(BODY_HEADING);
    body.push('\n');
    body.push_str(
        "\nAuthoritative signatures generated from the binary's clap model.\n\
         Every verb and flag is listed; a spelling not found here does not exist.\n\
         `<X>` required, `[X]` optional, `...` repeatable.\n",
    );
    for sub in cmd.get_subcommands().filter(|sub| !sub.is_hide_set()) {
        body.push_str(&format!("\n## maestro {}\n\n", sub.get_name()));
        let mut lines = Vec::new();
        collect_leaf_lines(sub, &format!("maestro {}", sub.get_name()), &mut lines);
        for line in lines {
            body.push_str(&line);
            body.push('\n');
        }
    }

    let digest = sha256_hex(body.as_bytes());
    format!(
        "<!-- maestro:cli-reference-version: {CLI_REFERENCE_FORMAT_VERSION} -->\n\
         <!-- maestro:cli-reference-sha256: {digest} -->\n\
         <!-- generated; do not edit by hand; regenerate: {REGENERATE_COMMAND} -->\n\
         {body}"
    )
}

/// Check the header's sha256 stamp against the body it covers, so a hand edit
/// is caught even without regenerating from the clap model.
pub fn verify_self_check(content: &str) -> Result<(), String> {
    let marker = "<!-- maestro:cli-reference-sha256: ";
    let stamp_start = content
        .find(marker)
        .ok_or_else(|| "missing sha256 stamp".to_string())?;
    let stamp = content[stamp_start + marker.len()..]
        .split(" -->")
        .next()
        .ok_or_else(|| "malformed sha256 stamp".to_string())?;
    let body_start = content
        .find(BODY_HEADING)
        .ok_or_else(|| format!("missing body heading {BODY_HEADING:?}"))?;
    let digest = sha256_hex(&content.as_bytes()[body_start..]);
    if stamp == digest {
        Ok(())
    } else {
        Err(format!(
            "self-check stamp {stamp} does not match body sha256 {digest}; \
             the file was hand-edited -- regenerate: {REGENERATE_COMMAND}"
        ))
    }
}

/// Append one `- \`signature\` -- about` bullet per leaf command under `cmd`.
fn collect_leaf_lines(cmd: &Command, path: &str, lines: &mut Vec<String>) {
    let subs: Vec<&Command> = cmd
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set())
        .collect();
    if subs.is_empty() {
        let about = cmd
            .get_about()
            .map(|about| format!(" -- {about}"))
            .unwrap_or_default();
        lines.push(format!("- `{}`{about}", signature(cmd, path)));
        return;
    }
    for sub in subs {
        collect_leaf_lines(sub, &format!("{path} {}", sub.get_name()), lines);
    }
}

/// One full invocation line: positionals in declaration order, then options.
fn signature(cmd: &Command, path: &str) -> String {
    let mut parts = vec![path.to_string()];
    for arg in cmd.get_positionals().filter(|arg| !arg.is_hide_set()) {
        let name = value_label(arg);
        let repeat = repeat_suffix(arg);
        if arg.is_required_set() {
            parts.push(format!("<{name}>{repeat}"));
        } else {
            parts.push(format!("[{name}]{repeat}"));
        }
    }
    for arg in cmd.get_arguments().filter(|arg| !arg.is_positional()) {
        if arg.is_hide_set() {
            continue;
        }
        let id = arg.get_id().as_str();
        if id == "help" || id == "version" {
            continue;
        }
        let mut flag = match (arg.get_short(), arg.get_long()) {
            (Some(short), Some(long)) => format!("-{short}|--{long}"),
            (None, Some(long)) => format!("--{long}"),
            (Some(short), None) => format!("-{short}"),
            (None, None) => continue,
        };
        if matches!(arg.get_action(), ArgAction::Set | ArgAction::Append) {
            flag.push_str(&format!(" <{}>", value_label(arg)));
        }
        let rendered = if arg.is_required_set() {
            flag
        } else {
            format!("[{flag}]")
        };
        parts.push(format!("{rendered}{}", repeat_suffix(arg)));
    }
    parts.join(" ")
}

fn repeat_suffix(arg: &Arg) -> &'static str {
    if matches!(arg.get_action(), ArgAction::Append | ArgAction::Count) {
        "..."
    } else {
        ""
    }
}

fn value_label(arg: &Arg) -> String {
    arg.get_value_names()
        .and_then(|names| names.first())
        .map(|name| name.to_string())
        .unwrap_or_else(|| arg.get_id().as_str().to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_covers_nested_verbs_with_full_flag_signatures() {
        let reference = render_cli_reference();
        assert!(reference.contains("## maestro task"), "{reference}");
        assert!(
            reference.contains("maestro task create <TITLE>"),
            "{reference}"
        );
        assert!(reference.contains("[--id-only]"), "{reference}");
        assert!(reference.contains("[--check <CHECK>]..."), "{reference}");
        assert!(
            reference.contains("maestro card show <ID>"),
            "the card namespace must be listed: {reference}"
        );
        assert!(
            reference.contains("-t|--type <TYPE>"),
            "short|long spelling with value: {reference}"
        );
    }

    #[test]
    fn every_top_level_command_gets_a_section() {
        let reference = render_cli_reference();
        let mut cmd = Cli::command();
        cmd.build();
        for sub in cmd.get_subcommands().filter(|sub| !sub.is_hide_set()) {
            assert!(
                reference.contains(&format!("## maestro {}", sub.get_name())),
                "missing section for {}",
                sub.get_name()
            );
        }
    }

    #[test]
    fn the_rendered_header_passes_its_own_self_check() {
        let reference = render_cli_reference();
        verify_self_check(&reference).expect("freshly rendered reference must self-check");
        assert!(
            reference.starts_with("<!-- maestro:cli-reference-version: "),
            "{reference}"
        );
    }

    #[test]
    fn a_hand_edit_fails_the_self_check() {
        let tampered = render_cli_reference().replace("## maestro task", "## maestro tasks");
        assert!(verify_self_check(&tampered).is_err());
    }
}
