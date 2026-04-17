# Specification Interview Questions

Generate a requirements specification through interactive questioning. Batch independent questions into a single interaction if your runtime supports it.

## Interview Strategy

**Batching:** Present all questions for a type in a single interaction. Users prefer answering everything at once over being drip-fed questions.

**Probing vague answers:** If an answer is one sentence or less, follow up. A single "add dark mode" answer to Q1 needs expansion -- ask "What elements get dark mode? Just the main app, or settings/modals/onboarding too? Does it follow system preference or is it a manual toggle?"

**Inferring from codebase:** Before asking Q2 (interaction type), scan the project. If it's a CLI tool with no UI framework, don't offer "UI component" as an option. If it's a React app with no CLI, don't offer "CLI command." Tailor the options to what's real.

**When to stop probing:** Accept the answer after one follow-up. If the user says "standard" or "none" twice, they mean it. Don't ask a third time.

---

## For Features

Batch these into a single interaction if your runtime supports it:

**Q1:** Ask the user: "What should this feature do? Describe the core behavior and expected outcomes."

Probe if vague: "Can you give me a concrete example of a user doing X and seeing Y?"

**Q2:** Ask the user: "How will users interact with this feature?"
Options:
- **UI component** -- Visual element users see and interact with
- **API endpoint** -- Programmatic interface
- **CLI command** -- Terminal command or flag
- **Background process** -- No direct user interaction

Auto-infer: Scan the project for UI frameworks, API routers, CLI argument parsers. Pre-select the most likely option and ask the user to confirm: "This looks like a {type} project -- is this feature also {type}?"

**Q3:** Ask the user: "Any constraints or non-functional requirements? (performance, security, compatibility)" (select all that apply)
Options:
- **No special constraints** -- Standard quality expectations
- **Performance-critical** -- Must meet specific latency/throughput targets
- **Security-sensitive** -- Handles auth, PII, or financial data
- **Let me specify** -- Type your constraints

**Q4:** Ask the user: "Any known edge cases or error scenarios to handle?"
Options:
- **I'll list them** -- Type known edge cases
- **Infer from requirements** -- Generate edge cases from the spec

If user selects "Infer," generate at least 4 edge cases from the requirements and present them for confirmation. Always include: empty/null input, concurrent access, network/IO failure, and permission/auth failure (where applicable).

**Q5:** Ask the user: "Does this touch existing features or modules? Which files/areas will be affected?"
Options:
- **New, standalone** -- No existing code affected
- **Extends existing** -- Builds on {module/feature} (ask which)
- **Cross-cutting** -- Touches multiple modules (ask which ones)

Auto-infer: If the description mentions an existing command, endpoint, or component name, search the codebase for it. Present findings: "I found `{file}` which looks related -- will this feature modify it?"

**Q6:** Ask the user: "What is explicitly out of scope? What should this feature NOT do?"
Options:
- **I'll list exclusions** -- Type what's out of scope
- **Nothing specific** -- You'll determine scope from requirements

This question is critical for preventing scope creep. If the user says "nothing specific," generate 2-3 likely out-of-scope items from the description and present them: "Just to confirm, these are NOT in scope: {list}. Correct?"

---

## For Bugs

Batch these into a single interaction if your runtime supports it:

**Q1:** Ask the user: "What is happening? Provide steps to reproduce."

Probe if vague: "Can you give numbered steps starting from a clean state? What do you see vs. what you expect?"

**Q2:** Ask the user: "What should happen instead?"

**Q3:** Ask the user: "How critical is this? Which users or flows are affected?"
Options:
- **Blocker** -- Core flow broken, no workaround
- **High** -- Significant degradation, workaround exists
- **Medium** -- Noticeable issue, limited impact
- **Low** -- Minor or cosmetic

**Q4:** Ask the user: "Do you know the root cause, or should we investigate?"
Options:
- **Known root cause** -- I can point to the code (ask for details)
- **Suspected area** -- I think it's in {module} (ask which)
- **Unknown** -- Needs investigation

Auto-infer: If the bug description mentions an error message, search the codebase for that exact string. Report findings: "I found this error string in `{file}:{line}` -- is that the right area?"

---

## For Chores

Batch these into a single interaction if your runtime supports it:

**Q1:** Ask the user: "What needs to change and why?"

Probe if vague: "What's the concrete problem with the current state? What breaks or degrades if we don't do this?"

**Q2:** Ask the user: "Any backward compatibility requirements?"
Options:
- **Must be backward compatible** -- No breaking changes to public API
- **Breaking changes acceptable** -- Semver major bump is fine
- **Internal only** -- No public surface affected

**Q3:** Ask the user: "What's the blast radius if this goes wrong? Which systems or consumers are affected?"
Options:
- **Isolated** -- Only affects this module, no downstream consumers
- **Moderate** -- Affects 2-3 dependent modules
- **Wide** -- Affects many consumers or external integrations
- **Not sure** -- Need to investigate (trigger a dependency scan)

If user selects "Not sure," scan for imports/references to the module being changed. Report the dependency count.

**Q4:** Ask the user: "How will you know this is done? What's the success criteria beyond 'it works'?"

Probe if vague: "Can you describe a before/after? A metric that changes? A test that now passes?"

---

## Spec Draft

Compose the spec from interview answers using `reference/spec-template.md` for structure.

**Draft quality gates** -- before presenting to the user, verify:
- Every interview answer maps to at least one spec section
- No interview answer was dropped or ignored
- Edge cases section has at least 3 items (generate more if the user provided fewer)
- Out of Scope section is populated (from Q6 for features, or inferred)
- Acceptance criteria are testable and specific (no "works correctly")

---

## Spec Approval Loop

Present the full draft to the user by embedding the entire spec content in the question field:

Ask the user: "Here is the drafted specification -- does it look correct?\n\n---\n{full spec content}\n---"
Options:
- **Approved** -- Spec is ready, generate the plan
- **Needs revision** -- I'll tell you what to change

### Revision Protocol

**Round 1-2:** Apply requested changes, re-present. Normal iteration.

**Round 3 (final):** If still not approved, summarize remaining disagreements and ask: "We've been through 3 rounds. Should I apply your latest feedback and finalize, or do we need to step back and reconsider the scope?" Accept whatever the user decides.

### When to Push Back

Push back (politely, once) when the user's revision:
- Adds scope that contradicts the Out of Scope section: "This was listed as out of scope -- should I move it in scope and update the plan accordingly?"
- Removes all edge cases: "I'd recommend keeping at least the error handling cases -- they prevent surprise failures during implementation."
- Makes acceptance criteria untestable: "How would we verify '{vague criterion}'? Can we rephrase it as a specific check?"

After pushing back once, accept the user's decision. Do not argue.

Write approved spec to `.maestro/tracks/{track_id}/spec.md`.

---

## Plan Generation

Read project context for informed planning:
- `.maestro/context/workflow.md`
- `.maestro/context/tech-stack.md`
- `.maestro/context/guidelines.md`

Use `reference/plan-template.md` for structure and rules (TDD injection, phase verification, sizing, dependencies).

Present the full plan for approval by embedding the entire plan content directly in the question field (same pattern as spec approval). Max 3 revision loops (same protocol as spec approval).

Write approved plan to `.maestro/tracks/{track_id}/plan.md`.
