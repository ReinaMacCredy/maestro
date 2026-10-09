# Verify skill

Use this recipe to create or maintain one repository's verification skill: the
procedure an agent follows to prove a change on the running product, not on a
proxy. It is how a claim reaches the `live` and `journey` layers in
maestro-verify. Pattern from pstack's create-verification-skill (poteto).

## Loop anatomy

### Perceive

Read the repository instructions, README, package scripts, and existing test
and e2e harnesses. Find how the product starts, what state it needs (env,
seed data, accounts, ports), and how a user reaches each feature. If a
`verify-<repo>` skill already exists, read it and compare it with the current
feature surface: this is the maintain pass.

### Choose

Pick the smallest set of journeys that covers the features an agent is likely
to touch. List every feature in a map (feature -> journey -> observable
outcome); mark a feature with no journey yet as `GAP`, never by omission.

### Act

Write `.agents/skills/verify-<repo>/SKILL.md` (committed; Codex loads it from
there) and link it for Claude Code with
`ln -s ../../.agents/skills/verify-<repo> .claude/skills/verify-<repo>`. Its
frontmatter description says when to load it: "verify a change in <repo> on
the running product". The body holds five sections:

1. Launch: the exact commands that start the real product from a clean state.
2. Doctor: how to confirm the instance is healthy and is the build under test
   (version, stamp, pid, port).
3. Drive: each journey as the user performs it (CLI invocations, HTTP calls,
   UI steps), against the real backend or a disposable copy of it.
4. Capture: for every step, the action, the resulting state, and side effects
   (files, rows, requests). Screenshots, recordings, or transcripts go to
   `maestro topic path verify-<repo>`; the proof survives cleanup.
5. Clean up: stop what Launch started and remove disposable state, never the
   captured proof.

Then the feature map from Choose.

### Observe

Run the skill once end to end before calling it done; a verify skill that
has never run is a draft. Record what each journey showed and any step that
failed or needed a manual fix, then fix the skill text.

### Learn

Update the feature map in the same change that adds or alters a feature. A
drift found later (a journey that no longer runs, a feature missing from the
map) is a maintain pass, not a new skill.

### Continue

Claims proven through the skill carry `(live)` or `(journey)` in their proof.
Report journeys still marked `GAP` with the layer they leave untested.
