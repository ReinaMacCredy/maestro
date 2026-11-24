# Verify skill

Use this recipe to create or maintain one repository's verification skill: the
procedure an agent follows to prove a change on the running product, not on a
proxy. It is how a claim reaches the `live` and `journey` layers in
maestro-verify. Folds rules from pstack's create-verification-skill and
maintain-verification-skill (Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Loop anatomy

### Perceive

Read the repository instructions, README, package scripts, and existing test
and e2e harnesses. Find how the product starts, what state it needs (env,
seed data, accounts, ports), and how a user reaches each feature. If a
`verify-<repo>` skill already exists, read it and compare it with the current
features. This is the maintain pass.

### Choose

Pick the smallest set of journeys that covers the features an agent is likely
to touch. List every feature in a map (feature -> journey -> observable
outcome); mark a feature with no journey yet as `GAP`, never by omission.

### Act

If the checkout does not build or start as it is, fix that first or report it
precisely. A skill written against a broken base teaches wrong steps. When an
irrelevant missing asset blocks startup (a static directory the API never
serves, a sample config), the skill may create it, marked as verification
scaffolding, and remove it at cleanup.

Write `.agents/skills/verify-<repo>/SKILL.md` (committed; Codex loads it from
there) and link it for Claude Code with
`ln -s ../../.agents/skills/verify-<repo> .claude/skills/verify-<repo>`. Its
frontmatter description says when to load it: "verify a change in <repo> on
the running product". The body holds five sections:

1. Launch: the exact commands that start the real product from a clean state,
   meaning an empty data directory with migrations and seed data applied, and
   the sign that it is ready. Ready means usable, not that a process answers.
   A database health check can pass while its init scripts still run. For a
   short-lived CLI, build once and start each drive in its own isolated
   session. If two instances cannot run side by side, say so, and never drive
   a shared running instance.
2. Doctor: how to confirm the instance is healthy and is the build under test
   (version, stamp, pid, port).
3. Drive: each journey as the user performs it (CLI invocations, HTTP calls,
   UI steps), against the real backend or a disposable copy of it.
4. Capture: for every step, the action, the resulting state, and side effects
   (files, rows, requests). Drive the real user path, never internal setters
   or test-only endpoints, and mock only where a production boundary already
   isolates the external system. When the safe path is a dry run or a test
   mode, observe what it skips (files, network, git refs) instead of trusting
   its name. Screenshots, recordings, or transcripts go to
   `maestro topic path verify-<repo>`; the proof survives cleanup.
5. Clean up: stop what Launch started, by the pid or handle it recorded and
   never by process name, and remove disposable state, never the captured
   proof.

Any script the skill ships is executable, and the skill body shows how to
invoke it.

Then the feature map from Choose. A journey whose precondition the seed data
lacks reports a failure, and the reason goes in that feature's row. Never
weaken a journey to make it pass. A proof that drives one convenient entry
point is incomplete when the map lists others.

### Observe

Run the skill once end to end before calling it done; a verify skill that
has never run is a draft. Launch, doctor, drive one mapped feature, capture,
and clean up. Then confirm the proof still exists where the skill says it
does. A cleanup that eats the proof fails this step. Run the cleanup after
every failed attempt too, so a broken attempt strands no process or port.
Record what each journey showed and any step that failed or needed a manual
fix, then fix the skill text. If the run used a stand-in for the real target
(a container for a fresh machine, a local stack for a remote one), list every
setup step the stand-in did that the target will not. Those steps stay
unproven until a run on the real target covers them.

### Learn

Update the feature map in the same change that adds or alters a feature. A
drift found later (a journey that no longer runs, a feature missing from the
map) is a maintain pass, not a new skill.

### Continue

Claims proven through the skill carry `(live)` or `(journey)` in their proof.
Report journeys still marked `GAP` with the layer they leave untested.

## Maintain pass

A feature map rots the moment the product changes. The maintain pass keeps it
honest. Its unit of rigor is the feature, not every sentence.

It ends in one of three outcomes, named in the report:

- clean: every feature got source and live coverage and nothing needs
  shipping. No branch.
- changed: one change of proven corrections.
- blocked: coverage could not finish, or a proven fix could not ship safely.
  Say exactly what blocked it.

Edit only the verify skill's own directory (its SKILL.md, its feature map and
the harness scripts it owns), never product code. A behavior the map
describes that the product no longer does is either drift, so fix the map, or
a product regression, so report it and never paper over it in the map.

1. Read the feature map and fix missing, extra, duplicate or dead entries.
2. Source wave. Launch one read-only subagent per feature on a mid
   model, all at once. Each explains from source how the feature
   works for its user, flags likely drift with citations, and returns one
   live-verification recipe. None drives the product or edits a file.
3. Reconcile. Every feature has a returned summary. Merge overlapping recipes
   into as few product states as practical. Spot-check the cited drift. Sweep
   recent changes for user-facing features the map lacks, and name a concrete
   source path before calling one missing.
4. Live pass, required even when the source looks clean. The coordinator does
   all the driving and follows the skill's own Launch model. Exercise every
   feature at least once, and hold three invariants for the whole pass:
   - Never drive an instance that has not passed doctor since it last did
     something surprising. Run doctor before the first drive, on each fresh
     session, and after any failed drive. Where doctor cannot see the failure,
     reset to a known state or relaunch.
   - Proof captured so far survives every cleanup, checked at its location.
   - Nothing a drive started outlives its use. On a shared instance, clean
     the residue, not the instance.
   A doctor failure caused by skill drift is drift. Fix it within the edit
   scope and retry once, restarting only what the fix invalidated, before
   calling the pass blocked. A feature counts as `verified-unreachable` only
   with the concrete prerequisite (auth, entitlement, OS, external state) and
   the route attempted; a prerequisite the map omits is drift. Re-drive every
   harness fix live before it ships. Tear down after the last drive, re-proofs
   included.
5. Triage. A wrong or missing description is drift; fix it. Working behavior
   the harness cannot drive is a harness gap; fix it under the script rule in
   Act. Broken product behavior is a product gap; record it as a finding for
   the owner and keep it out of this change.
6. Ship or stop. For changed, re-read every changed file, then deliver through
   `maestro recipe show opening-a-pr`, which pushes and opens a PR only under
   the delivery grant. For clean or blocked, report the outcome and the
   coverage. Run notes (features covered, unreachable prerequisites, confirmed
   drift, outcome) stay in `maestro topic path verify-<repo>` and are never
   committed.
