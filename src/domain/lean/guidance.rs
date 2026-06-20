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

/// Mode-adjusted guidance for `maestro lean review`: walk a diff against the
/// reach-ladder. `off` short-circuits to a one-line skip.
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
         For each unit of new code, find the lowest rung that already covers it.\n\
         {directive}\n\n\
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
         Sweep the surface for code a lower reach-ladder rung already covers:\n  \
         {LADDER}\n\n\
         Anchor each finding with a comment marker (a `//`, `#`, or `--` line whose\n\
         first token is `lean:`, then the note) so `maestro lean debt` harvests it.\n\
         {directive}\n\n\
         This rides the existing maestro-audit pass; it does not replace it.\n",
        directive = climb_directive(mode),
    )
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
