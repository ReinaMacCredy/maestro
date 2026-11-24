# Authoring a skill

Use this recipe to write or change an agent skill, meaning a SKILL.md and the
references it links. You own the skill's voice. Ported from pstack's
authoring-a-skill playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Steps

1. Pick the route by the kind of skill.
   - An edit to an existing skill maestro ships (`src/plugins/skills/maestro-*`
     in the maestro repository) is a doctrine edit. File the correction with
     `maestro lesson file`, make the smallest edit that would have prevented
     it, run `bun test tests/scenario-golden.test.ts` and every suite that
     asserts the changed text, and have a separate reviewer challenge the
     edit, on top of the checks in step 4.
   - A repository's verify-<repo> skill follows
     `maestro recipe show verification-skill`.
   - Any other skill, a new maestro skill included, follows the steps below.
2. Write the skill under the style rules below. The frontmatter carries
   `name`, equal to the directory name, and a `description` that says when to
   load it. A skill maestro ships also carries `review-date: <YYYY-MM-DD>` in
   its frontmatter and the line `<!-- maestro-skill-version: dev -->` right
   after the frontmatter.
3. For a skill maestro ships, hand the code to a subagent under the `delegate`
   profile. That covers the entry in `skillNames` in `src/plugins/skills.ts`,
   the skill and reference list in `tests/workflow-plugin.test.ts`, and a
   `[lint]` test in `tests/docs-contract.test.ts`. Add an assertion only for a
   structural rule, a shape or a command a check can assert. A subjective rule
   gets none (`~/maestro/WORKFLOW.md#testing-discipline`).
4. Validate against the checks maestro runs.
   - Version gate. Install copies and links for Claude and Codex only the
     skills named in `skillNames`. It replaces `dev` in the stamp with the
     installing commit, and it treats an installed SKILL.md without the stamp
     as the owner's file and never overwrites it. A source without the stamp
     installs once, and every later install skips it.
   - Review date. `maestro doctor` reports a `review-date` that is past due.
   - Docs contract. A skill's `[lint]` test asserts that the frontmatter opens
     with `name: <skill>`, that the stamp is present, and that every
     `references/<file>` the SKILL.md links exists and is linked. It quotes
     each load-bearing sentence, so a later edit cannot drop one silently.
   - Links. Every recipe the text names resolves with
     `maestro recipe show <name>`, and every skill it names exists.
   - Run `bun test tests/docs-contract.test.ts tests/workflow-plugin.test.ts`.
     Grep the test tree for each sentence you changed and run every suite that
     quotes it.
   - Prove the installed copy through the repository's verify-<repo> skill
     (`maestro recipe show verification-skill`). In a scratch HOME the
     installed SKILL.md carries the installing commit in its stamp.
5. Deliver: `maestro recipe show opening-a-pr`.

## Style

When in doubt, delete. Keep only prose that changes a decision. Tell the
reader to do the thing and skip the reason. Explain only when the rule is
confusing without one. Match tone to scope. Point at structural sources
(types, READMEs, config) instead of restating them, per
**Encode Lessons in Structure** (PRINCIPLES.md). Delegate to other skills and
recipes by name, and point at a rule `~/maestro/WORKFLOW.md` owns instead of
copying it. A workflow you keep hitting that no skill captures is a proposal
for a new skill.

## Reply

A summary of the skill, the key design decisions, and the validation notes,
naming each check that ran and its count.
