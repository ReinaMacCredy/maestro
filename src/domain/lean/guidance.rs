//! Lean guidance: the mode-adjusted prompts `maestro lean review` and
//! `maestro lean audit` emit. Both walk the same reach-ladder; the session's
//! [`LeanMode`] tunes the climb directive (ultra rejects, full applies, lite
//! suggests, off skips). This is thin agent guidance, not an analyzer.

use crate::domain::lean::LeanMode;

/// The reach-ladder, lowest rung first: reach for the cheapest before the next.
const LADDER: &str = "skip/YAGNI -> stdlib -> native platform -> installed dependency -> one-liner -> minimal new code";

/// What the session mode tells the agent to do with a unit the ladder already
/// covers from a lower rung.
fn climb_directive(mode: LeanMode) -> &'static str {
    match mode {
        LeanMode::Ultra => {
            "ultra: reject code a lower rung already covers; send it back rather than apply it."
        }
        LeanMode::Full => "full: apply the cheaper, lower-rung version in place.",
        LeanMode::Lite => "lite: suggest the cheaper version; leave the call to the author.",
        LeanMode::Off => "off: the climb step is suppressed this session; skip it.",
    }
}

/// The finding tags `review` and `audit` emit, each with what it flags and the
/// replacement to show. Over-engineering only; correctness/security/perf are out.
const TAGS: &str = "delete: (dead or speculative code; replacement is removal), \
     stdlib: (the stdlib already does it; name the call), \
     native: (a platform or language feature covers it; name it), \
     yagni: (an abstraction with one caller; inline it), \
     shrink: (same behavior in fewer lines; show the shorter form)";

/// Mode-adjusted guidance for `maestro lean review`: walk a diff against the
/// reach-ladder and list tagged findings. `off` short-circuits to a one-line skip.
pub fn review_guidance(mode: LeanMode) -> String {
    if mode == LeanMode::Off {
        return format!(
            "Lean review (mode: {mode})\n\n\
             The reach-ladder climb step is suppressed this session; skip it.\n\
             Switch with `maestro lean <lite|full|ultra>`.\n"
        );
    }
    format!(
        "Lean review (mode: {mode})\n\n\
         Walk the diff against the reach-ladder, lowest rung first:\n  \
         {LADDER}\n\n\
         For each over-built unit, write one finding line:\n  \
         L<line>: <tag> <what>. <replacement>.\n\
         Tags: {TAGS}.\n\
         Close with `net: -<N> lines possible.`, or `Lean already. Ship.` when\n\
         nothing is over-built.\n\n\
         Over-engineering only: leave correctness, security, and performance to\n\
         other passes, and never flag a validation, error-handling, or security\n\
         line or the one runnable check. List every finding; {directive}\n\n\
         This is the same cleanup the simplify reference applies; it does not replace it.\n",
        directive = climb_directive(mode),
    )
}

/// Mode-adjusted guidance for `maestro lean audit`: sweep the tree against the
/// reach-ladder and anchor findings as `// lean:` markers. `off` short-circuits.
pub fn audit_guidance(mode: LeanMode) -> String {
    if mode == LeanMode::Off {
        return format!(
            "Lean audit (mode: {mode})\n\n\
             The reach-ladder climb step is suppressed this session; skip it.\n\
             Switch with `maestro lean <lite|full|ultra>`.\n"
        );
    }
    format!(
        "Lean audit (mode: {mode})\n\n\
         Sweep the tree for code a lower reach-ladder rung already covers:\n  \
         {LADDER}\n\n\
         Hunt for: a dependency the stdlib ships; an interface or trait with one\n\
         impl; a factory with one product; a wrapper that only delegates; a module\n\
         that exports one thing; dead flags or config; hand-rolled stdlib.\n\
         Tag each finding with {TAGS}, and rank biggest cut first.\n\
         Anchor each finding with a comment marker (a `//`, `#`, or `--` line whose\n\
         first token is `lean:`, then the note) so `maestro lean debt` harvests it.\n\
         Close with `net: -<N> lines, -<M> deps possible.`.\n\
         List every finding; {directive}\n\n\
         This rides the existing maestro-audit pass; it does not replace it.\n",
        directive = climb_directive(mode),
    )
}

/// The `maestro lean help` card: the modes, the verbs, and how to set a default.
/// Static text, no session lookup -- a quick orientation to the lean surface.
pub fn help_guidance() -> String {
    "maestro lean -- reach-ladder strictness for this session\n\n\
     Modes (set with `maestro lean <mode>`):\n  \
     lite   suggest the cheaper version; leave the call to the author\n  \
     full   apply the cheaper, lower-rung version in place (default)\n  \
     ultra  reject code a lower rung already covers\n  \
     off    suppress the proactive climb step this session\n\n\
     Verbs:\n  \
     maestro lean                print the session's current mode\n  \
     maestro lean review         tagged reach-ladder findings over a diff\n  \
     maestro lean audit          tagged reach-ladder findings over the tree\n  \
     maestro lean debt [--card]  list `// lean:` markers (and mint cards)\n  \
     maestro lean help           this card\n\n\
     Default mode: set MAESTRO_LEAN=<mode> for new sessions (else full).\n"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIMB_VERBS: [(LeanMode, &str); 4] = [
        (LeanMode::Ultra, "reject"),
        (LeanMode::Full, "apply"),
        (LeanMode::Lite, "suggest"),
        (LeanMode::Off, "skip"),
    ];

    #[test]
    fn review_names_the_mode_and_its_climb_directive() {
        for (mode, verb) in CLIMB_VERBS {
            let text = review_guidance(mode);
            assert!(
                text.contains(mode.as_str()),
                "review for {mode} names the mode"
            );
            assert!(
                text.to_lowercase().contains(verb),
                "review for {mode} carries the `{verb}` climb directive: {text}"
            );
        }
    }

    #[test]
    fn audit_names_the_mode_and_its_climb_directive() {
        for (mode, verb) in CLIMB_VERBS {
            let text = audit_guidance(mode);
            assert!(
                text.contains(mode.as_str()),
                "audit for {mode} names the mode"
            );
            assert!(
                text.to_lowercase().contains(verb),
                "audit for {mode} carries the `{verb}` climb directive: {text}"
            );
        }
    }

    #[test]
    fn review_keeps_the_simplify_name_and_audit_keeps_the_maestro_audit_name() {
        assert!(
            review_guidance(LeanMode::Full).contains("simplify"),
            "review points at the existing simplify reference by name"
        );
        assert!(
            audit_guidance(LeanMode::Full).contains("maestro-audit"),
            "audit points at the existing maestro-audit skill by name"
        );
    }

    #[test]
    fn review_lists_the_tag_taxonomy_and_a_net_total() {
        let text = review_guidance(LeanMode::Full);
        for tag in ["delete:", "stdlib:", "native:", "yagni:", "shrink:"] {
            assert!(text.contains(tag), "review names the `{tag}` tag: {text}");
        }
        assert!(
            text.contains("net:"),
            "review closes with a net total: {text}"
        );
        assert!(
            text.contains("L<line>:"),
            "review gives the per-finding line form: {text}"
        );
    }

    #[test]
    fn audit_lists_the_hunt_the_tags_and_a_net_total() {
        let text = audit_guidance(LeanMode::Full);
        assert!(text.contains("Hunt"), "audit names the hunt list: {text}");
        for tag in ["delete:", "yagni:", "shrink:"] {
            assert!(text.contains(tag), "audit names the `{tag}` tag: {text}");
        }
        assert!(
            text.contains("deps possible"),
            "audit closes with a net lines+deps total: {text}"
        );
    }

    #[test]
    fn help_names_every_mode_every_verb_and_the_default_env() {
        let text = help_guidance();
        for mode in ["lite", "full", "ultra", "off"] {
            assert!(text.contains(mode), "help names the `{mode}` mode: {text}");
        }
        for verb in ["review", "audit", "debt", "help"] {
            assert!(text.contains(verb), "help names the `{verb}` verb: {text}");
        }
        assert!(
            text.contains("MAESTRO_LEAN"),
            "help names the default-mode env var: {text}"
        );
    }

    #[test]
    fn off_skips_the_taxonomy_for_both() {
        for text in [
            review_guidance(LeanMode::Off),
            audit_guidance(LeanMode::Off),
        ] {
            assert!(!text.contains("net:"), "off short-circuits before findings");
            assert!(text.contains("suppressed"), "off names the suppression");
        }
    }

    #[test]
    fn both_carry_the_reach_ladder_rungs() {
        for text in [
            review_guidance(LeanMode::Full),
            audit_guidance(LeanMode::Full),
        ] {
            assert!(
                text.contains("stdlib"),
                "ladder names the stdlib rung: {text}"
            );
            assert!(
                text.contains("one-liner"),
                "ladder names the one-liner rung: {text}"
            );
        }
    }
}
