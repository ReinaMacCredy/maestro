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
pub const CLI_REFERENCE_FORMAT_VERSION: &str = "1.1.0";

/// First body line; everything from here on is covered by the header's
/// sha256 self-check stamp.
const BODY_HEADING: &str = "# maestro CLI reference";

/// The one command that rewrites the committed copies.
pub const REGENERATE_COMMAND: &str =
    "cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored";

/// Render the full reference: a version-stamped self-checking header plus one
/// signature bullet per invokable command.
pub fn render_cli_reference() -> String {
    render_cli_reference_with_scope(None, CommandScope::full())
}

/// Render the skill-specific reference for one bundled Maestro skill.
///
/// Unknown skill names fall back to the full reference so the renderer remains
/// usable for diagnostics outside the bundled catalog.
pub fn render_cli_reference_for_skill(skill: &str) -> String {
    render_cli_reference_with_scope(Some(skill), CommandScope::for_skill(skill))
}

fn render_cli_reference_with_scope(skill: Option<&str>, scope: CommandScope) -> String {
    let mut cmd = Cli::command();
    cmd.build();

    let mut body = String::new();
    body.push_str(BODY_HEADING);
    body.push('\n');
    match skill {
        Some(skill) => body.push_str(&format!(
            "\nAuthoritative signatures generated from the binary's clap model,\n\
             filtered for the `{skill}` skill. Every listed verb and flag is exact;\n\
             a spelling not found here is outside this skill's CLI surface.\n\
             `<X>` required, `[X]` optional, `...` repeatable.\n"
        )),
        None => body.push_str(
            "\nAuthoritative signatures generated from the binary's clap model.\n\
             Every verb and flag is listed; a spelling not found here does not exist.\n\
             `<X>` required, `[X]` optional, `...` repeatable.\n",
        ),
    }
    for sub in cmd.get_subcommands().filter(|sub| is_rendered(sub)) {
        let mut lines = Vec::new();
        let path = format!("maestro {}", sub.get_name());
        if !scope.visits(&path) {
            continue;
        }
        collect_invocation_lines(sub, &path, &mut lines, scope);
        if lines.is_empty() {
            continue;
        }
        body.push_str(&format!("\n## maestro {}\n\n", sub.get_name()));
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

#[derive(Clone, Copy)]
struct CommandScope {
    paths: Option<&'static [&'static str]>,
}

impl CommandScope {
    fn full() -> Self {
        Self { paths: None }
    }

    fn for_skill(skill: &str) -> Self {
        let paths = match skill {
            "maestro-audit" => Some(MAESTRO_AUDIT_COMMANDS),
            "maestro-card" => Some(MAESTRO_CARD_COMMANDS),
            "maestro-design" => Some(MAESTRO_DESIGN_COMMANDS),
            "maestro-setup" => Some(MAESTRO_SETUP_COMMANDS),
            _ => None,
        };
        Self { paths }
    }

    fn includes(&self, path: &str) -> bool {
        self.paths
            .map(|paths| paths.contains(&path))
            .unwrap_or(true)
    }

    fn visits(&self, path: &str) -> bool {
        self.paths
            .map(|paths| {
                let descendant_prefix = format!("{path} ");
                paths
                    .iter()
                    .any(|allowed| *allowed == path || allowed.starts_with(&descendant_prefix))
            })
            .unwrap_or(true)
    }
}

const MAESTRO_CARD_COMMANDS: &[&str] = &[
    "maestro status",
    "maestro active",
    "maestro loop list",
    "maestro loop show",
    "maestro loop next",
    "maestro loop work-lease",
    "maestro task add",
    "maestro task create",
    "maestro task set",
    "maestro task explore",
    "maestro task accept",
    "maestro task claim",
    "maestro task start",
    "maestro task done",
    "maestro task complete",
    "maestro task verify",
    "maestro task next",
    "maestro task note",
    "maestro task update",
    "maestro task block",
    "maestro task unblock",
    "maestro task reject",
    "maestro task abandon",
    "maestro task supersede",
    "maestro task show",
    "maestro task list",
    "maestro task watch",
    "maestro task proof",
    "maestro task doctor",
    "maestro event create",
    "maestro event intervention",
    "maestro feature finalize",
    "maestro feature accept",
    "maestro feature prepare",
    "maestro feature amend",
    "maestro feature start",
    "maestro feature verify",
    "maestro feature proof add",
    "maestro feature proof waive",
    "maestro feature note",
    "maestro feature close",
    "maestro feature cancel",
    "maestro feature show",
    "maestro feature list",
    "maestro feature archive",
    "maestro feature auto-archive",
    "maestro feature unarchive",
    "maestro worktree plan",
    "maestro worktree mark",
    "maestro worktree cleanup-record",
    "maestro qa baseline",
    "maestro qa slice",
    "maestro memory create",
    "maestro memory list",
    "maestro memory show",
    "maestro memory search",
    "maestro memory promote",
    "maestro memory maintain",
    "maestro memory dream",
    "maestro memory scorer attach",
    "maestro memory suggest list",
    "maestro memory suggest create",
    "maestro memory suggest dismiss",
    "maestro scorer run",
    "maestro scorer show",
    "maestro scorer list",
    "maestro card ready",
    "maestro card list",
    "maestro card dep add",
    "maestro card dep remove",
    "maestro card archive",
    "maestro card claim",
    "maestro card assign",
    "maestro card note",
    "maestro card create",
    "maestro card show",
    "maestro card update",
    "maestro card close",
    "maestro card graph",
    "maestro harness list",
    "maestro harness show",
    "maestro harness apply",
    "maestro harness measure",
    "maestro harness dismiss",
    "maestro link add",
    "maestro link remove",
    "maestro msg send",
    "maestro msg read",
    "maestro msg list",
    "maestro conflict",
    "maestro watch",
    "maestro watch snapshot",
];

const MAESTRO_DESIGN_COMMANDS: &[&str] = &[
    "maestro status",
    "maestro active",
    "maestro loop list",
    "maestro loop show",
    "maestro loop next",
    "maestro feature new",
    "maestro feature set",
    "maestro feature finalize",
    "maestro feature show",
    "maestro feature spec",
    "maestro feature list",
    "maestro decision new",
    "maestro decision lock",
    "maestro decision supersede",
    "maestro decision show",
    "maestro decision list",
    "maestro card list",
    "maestro card show",
    "maestro link add",
    "maestro link remove",
    "maestro msg send",
    "maestro msg read",
    "maestro msg list",
    "maestro design list",
    "maestro design init",
];

const MAESTRO_AUDIT_COMMANDS: &[&str] = &[
    "maestro status",
    "maestro active",
    "maestro loop list",
    "maestro loop show",
    "maestro loop next",
    "maestro task show",
    "maestro task list",
    "maestro feature show",
    "maestro feature list",
    "maestro decision show",
    "maestro decision list",
    "maestro card list",
    "maestro card show",
    "maestro harness list",
    "maestro harness show",
    "maestro harness propose",
    "maestro harness apply",
    "maestro lean",
    "maestro query matrix",
    "maestro query friction",
    "maestro query backlog",
];

const MAESTRO_SETUP_COMMANDS: &[&str] = &[
    "maestro init",
    "maestro install",
    "maestro upgrade",
    "maestro sync",
    "maestro uninstall",
    "maestro doctor",
    "maestro shell-init",
    "maestro status",
    "maestro active",
    "maestro loop list",
    "maestro loop show",
    "maestro loop next",
];

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

/// Append one `- \`signature\` -- about` bullet per invokable command under `cmd`.
fn collect_invocation_lines(
    cmd: &Command,
    path: &str,
    lines: &mut Vec<String>,
    scope: CommandScope,
) {
    let subs: Vec<&Command> = cmd
        .get_subcommands()
        .filter(|sub| is_rendered(sub))
        .collect();
    if scope.includes(path) && (subs.is_empty() || has_visible_invocation_args(cmd)) {
        lines.push(signature_line(cmd, path));
        if subs.is_empty() {
            return;
        }
    }
    for sub in subs {
        let child_path = format!("{path} {}", sub.get_name());
        if scope.visits(&child_path) {
            collect_invocation_lines(sub, &child_path, lines, scope);
        }
    }
}

/// A subcommand is rendered unless it is clap-hidden or the auto-generated
/// `help` pseudo-subcommand, which only duplicates its siblings under
/// `<ns> help <verb>` and is pure noise in an agent-facing reference.
fn is_rendered(sub: &Command) -> bool {
    !sub.is_hide_set() && sub.get_name() != "help"
}

fn has_visible_invocation_args(cmd: &Command) -> bool {
    cmd.get_arguments().any(|arg| {
        !arg.is_hide_set() && {
            let id = arg.get_id().as_str();
            id != "help" && id != "version"
        }
    })
}

fn signature_line(cmd: &Command, path: &str) -> String {
    let about = cmd
        .get_about()
        .map(|about| format!(" -- {about}"))
        .unwrap_or_default();
    format!("- `{}`{about}", signature(cmd, path))
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
            reference.contains("maestro watch [ID] [--interval <INTERVAL>]"),
            "invokable parents with args must be listed: {reference}"
        );
        assert!(
            !reference.contains("- `maestro task`"),
            "pure command namespaces must not be listed as invokable commands: {reference}"
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
        for sub in cmd.get_subcommands().filter(|sub| is_rendered(sub)) {
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

    #[test]
    fn hidden_duplicates_and_relocations_shape_the_reference() {
        let reference = render_cli_reference();
        // The 11 flat card verbs + top-level verify + both migrations are hidden:
        // no top-level section renders for them.
        for hidden in [
            "## maestro ready",
            "## maestro list",
            "## maestro dep",
            "## maestro archive",
            "## maestro claim",
            "## maestro assign",
            "## maestro note",
            "## maestro create",
            "## maestro show",
            "## maestro update",
            "## maestro close",
            "## maestro verify",
            "## maestro migrate",
            "## maestro migrate-v2",
        ] {
            assert!(
                !reference.contains(hidden),
                "hidden top-level verb still rendered: {hidden}\n{reference}"
            );
        }
        // The card namespace is the canonical home for those verbs.
        assert!(reference.contains("## maestro card"), "{reference}");
        assert!(reference.contains("maestro card ready"), "{reference}");
        // Relocated read views are canonical under their resource.
        assert!(
            reference.contains("maestro task proof"),
            "task proof must be canonical: {reference}"
        );
        assert!(
            reference.contains("maestro card graph"),
            "card graph must be canonical: {reference}"
        );
        // query keeps the cross-cutting reports and drops the relocated/synonym ones.
        assert!(reference.contains("maestro query matrix"), "{reference}");
        assert!(reference.contains("maestro query friction"), "{reference}");
        assert!(reference.contains("maestro query backlog"), "{reference}");
        for gone in [
            "maestro query proof",
            "maestro query graph",
            "maestro query decisions",
        ] {
            assert!(
                !reference.contains(gone),
                "relocated/synonym query verb must be hidden: {gone}\n{reference}"
            );
        }
        // mcp shows serve/tools only; the stdin/list synonyms are hidden.
        assert!(reference.contains("maestro mcp serve"), "{reference}");
        assert!(reference.contains("maestro mcp tools"), "{reference}");
        assert!(!reference.contains("maestro mcp stdin"), "{reference}");
        assert!(!reference.contains("maestro mcp list"), "{reference}");
    }

    #[test]
    fn skill_scopes_render_filtered_exact_signatures() {
        let card = render_cli_reference_for_skill("maestro-card");
        assert!(card.contains("maestro task add <TITLE>"), "{card}");
        assert!(card.contains("maestro task start <REF_OR_ID>"), "{card}");
        assert!(card.contains("maestro task done <REF_OR_ID>"), "{card}");
        assert!(card.contains("maestro task complete <ID>"), "{card}");
        assert!(card.contains("maestro feature finalize <ID>"), "{card}");
        assert!(card.contains("maestro feature prepare <ID>"), "{card}");
        assert!(card.contains("maestro qa slice <ID>"), "{card}");
        assert!(card.contains("maestro memory"), "{card}");
        assert!(card.contains("maestro scorer"), "{card}");
        assert!(card.contains("maestro card graph"), "{card}");
        assert!(!card.contains("maestro init"), "{card}");
        assert!(!card.contains("maestro mcp serve"), "{card}");
        assert!(!card.contains("maestro harness propose"), "{card}");

        let design = render_cli_reference_for_skill("maestro-design");
        assert!(design.contains("maestro feature spec <ID>"), "{design}");
        assert!(design.contains("maestro feature finalize <ID>"), "{design}");
        assert!(design.contains("maestro decision lock <ID>"), "{design}");
        assert!(design.contains("maestro card show <ID>"), "{design}");
        assert!(!design.contains("maestro task complete"), "{design}");
        assert!(!design.contains("maestro qa baseline"), "{design}");

        let audit = render_cli_reference_for_skill("maestro-audit");
        assert!(audit.contains("maestro harness propose"), "{audit}");
        assert!(audit.contains("maestro harness apply"), "{audit}");
        assert!(audit.contains("maestro lean [TARGET]"), "{audit}");
        assert!(audit.contains("maestro query friction"), "{audit}");
        assert!(audit.contains("maestro card show <ID>"), "{audit}");
        assert!(!audit.contains("maestro card update"), "{audit}");

        let setup = render_cli_reference_for_skill("maestro-setup");
        assert!(setup.contains("maestro init [--dry-run]"), "{setup}");
        assert!(setup.contains("maestro install"), "{setup}");
        assert!(setup.contains("maestro doctor"), "{setup}");
        assert!(!setup.contains("maestro task claim"), "{setup}");
        assert!(!setup.contains("maestro decision new"), "{setup}");
    }

    #[test]
    fn clap_auto_help_pseudo_subcommands_are_not_rendered() {
        let reference = render_cli_reference();
        assert!(
            !reference.contains("## maestro help"),
            "the root `help` pseudo-subcommand must not get a section: {reference}"
        );
        for line in reference.lines() {
            let Some(rest) = line.strip_prefix("- `maestro ") else {
                continue;
            };
            let path = rest.split('`').next().unwrap_or_default();
            assert!(
                !path.split_whitespace().any(|tok| tok == "help"),
                "clap auto-`help` pseudo-subcommand lines must not render: {line}"
            );
        }
    }
}
