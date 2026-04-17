# Setup Interview Questions

Each interview step starts with an **Interactive vs Autogenerate** choice. The choice is context-dependent -- not always offered.

## When to Offer Autogenerate

```
Autogenerate is available ONLY when:
  1. Project is brownfield, AND
  2. Scan was permitted, AND
  3. Scan produced strong inferences for this specific file

If ANY condition is false: show only "Interactive" (and "Skip" where noted).
Do NOT offer "Autogenerate from nothing" -- that produces placeholder garbage.
```

## Interview Flow Per Step

Every interview step follows this protocol:

1. **Check inferences.** Do you have scan data relevant to this file?
2. **Present the choice.** Autogenerate (if available) or Interactive. Some steps also offer Skip.
3. **Ask questions.** If interactive, ask sequentially. Pre-fill defaults from inferences when available.
4. **Generate the file.** Write using the template from `reference/templates.md`.
5. **Verify with user.** Display the file. Ask for corrections. Loop until confirmed.
6. **Write state.** Record completion in `setup_state.json`.

---

## Product Definition Interview (Step 6)

Generates `.maestro/context/product.md`.

### Choice

Ask the user: "How would you like to provide the product definition?"
Options:
- **Interactive** -- Answer questions step by step
- **Autogenerate** -- I'll infer everything from the codebase analysis _(only if scan data exists)_

If **Autogenerate**: Draft the file from README, package description, and manifest metadata. Present for confirmation. If user says "needs changes", switch to Interactive with the draft as starting values.

### Interactive Questions

Ask sequentially. **Maximum 3 questions** -- do not add more.

**Q1 -- Project purpose:**

Ask the user: "What does this project do? (one sentence)"

_Pre-fill logic:_
- Brownfield with README: Extract the first meaningful sentence. Present as default.
- Brownfield with `package.json` `description` field: Use that.
- Greenfield: No default. Ask directly.

Options:
- **{inferred purpose}** -- Based on README/package analysis _(only if inference available)_
- **Let me describe it** -- Type your own description

**Q2 -- Target users:**

Ask the user: "Who are the primary users?"

_Auto-inference:_ If the project has a CLI entry point or ships as a package, default to "Developers". If it has a `pages/` or `app/` directory with UI components, default to "End users".

Options:
- **Developers** -- Library, CLI tool, or developer-facing API
- **End users** -- Web app, mobile app, or consumer-facing product
- **Internal team** -- Internal tool, admin dashboard, or ops tooling

**Q3 -- Key features:**

_Skip condition:_ Brownfield project with a README that has a clear features section (bullet list under "Features", "What it does", or similar heading). In that case, extract features from README and present for confirmation instead of asking.

Ask the user: "What are the 2-3 most important features or capabilities?"
Options:
- **Auto-generate from analysis** -- I'll infer from the codebase _(only if scan data exists)_
- **Let me list them** -- Type your own list

### Verification

Display the generated `product.md`. Ask: "Does this accurately describe your project?"
- Confirmed --> write state, continue.
- Needs changes --> ask what to change, edit, re-display.

---

## Tech Stack Interview (Step 7)

Generates `.maestro/context/tech-stack.md`.

### Choice

Ask the user: "How would you like to provide the tech stack?"
Options:
- **Interactive** -- Review and confirm the detected stack or enter it manually
- **Autogenerate** -- I'll infer the full tech stack from config files _(only if scan data exists)_

### Interactive: Brownfield Path

Present the inferred stack as a structured summary:

```
Detected tech stack:
  Languages:   TypeScript, Python
  Frameworks:  Next.js, FastAPI
  Testing:     Jest, Pytest
  Package mgr: bun, uv
  Database:    PostgreSQL (from docker-compose)
  CI/CD:       GitHub Actions
```

Ask the user: "Is this your tech stack?"
Options:
- **Yes, correct** -- Use the detected tech stack
- **Needs changes** -- Let me correct or add to it

If "Needs changes": Ask what to add, remove, or correct. Apply changes.

_Common misdetections to watch for:_
- Shell scripts detected as a "language" -- usually incidental (CI scripts). Ask before including.
- Dev dependencies detected as production frameworks -- check if they appear in `devDependencies` vs `dependencies`.
- Multiple package managers detected -- ask which is primary.

### Interactive: Greenfield Path

No inferences available. Ask directly:

Ask the user: "What tech stack will this project use?"

Sub-questions (ask only what's relevant):
1. "Primary language(s)?"
2. "Framework(s)?" _(skip if pure library/CLI)_
3. "Package manager?" _(default to standard for the language: bun for TS/JS, uv for Python, cargo for Rust, go modules for Go)_
4. "Database?" _(skip if no persistence needed)_
5. "Testing framework?" _(default to standard: bun test, pytest, cargo test)_

### Verification

Display the generated `tech-stack.md`. Confirm with user. Pay special attention to:
- Are the **development commands** correct? (`test`, `lint`, `build`, `dev`)
- Is the **package manager** right? (wrong package manager = broken `maestro:implement`)

---

## Coding Guidelines Interview (Step 8)

Generates `.maestro/context/guidelines.md`.

### Choice

Ask the user: "How would you like to define coding guidelines?"
Options:
- **Interactive** -- Select from common principles and conventions
- **Autogenerate** -- I'll infer from CLAUDE.md, linter configs, and conventions _(only if scan data exists)_

### Auto-inference Sources

When autogenerating, pull from these sources in priority order:

| Source | What to extract |
|--------|-----------------|
| `CLAUDE.md` | Development principles, hard rules, testing requirements |
| `.eslintrc*` / `biome.json` / `ruff.toml` | Style rules, enabled rule sets |
| `.prettierrc*` | Formatting preferences |
| `tsconfig.json` strict flags | Type strictness level |
| `package.json` scripts | Available lint/test/format commands |
| `.editorconfig` | Indentation, line endings |

**Conflict resolution:** If CLAUDE.md says "no any types" but tsconfig has `strict: false`, flag the conflict and ask the user which is authoritative.

### Interactive Questions

Ask the user: "Any specific coding guidelines or principles for this project?" (select all that apply)
Options:
- **TDD-first** -- Test-driven development, high coverage
- **Move fast** -- Ship quickly, iterate later
- **Security-first** -- Input validation, audit logging, secure defaults
- **Accessibility-first** -- WCAG compliance, semantic HTML, screen reader support
- **Let me describe** -- Type custom guidelines

Follow-up (if not covered by selection):

Ask the user: "Any conventions I should know about?"
_Pre-fill:_ If CLAUDE.md or linter configs exist, list detected conventions and ask for confirmation.
Options:
- **Use detected conventions** -- {summary of what was found} _(only if detected)_
- **Let me add specifics** -- Type additional conventions
- **No additional conventions** -- Use the selected principles only

### Verification

Display `guidelines.md`. Check:
- Do the guidelines match what the user actually does? (Not aspirational rules they ignore.)
- Are there contradictions between guidelines and existing code? (Flag them.)

---

## Product Guidelines Interview (Step 9)

Generates `.maestro/context/product-guidelines.md`.

### Skip Condition

This step is skippable. Offer "Skip" for projects where product guidelines are irrelevant:
- Pure libraries with no UI or user-facing output
- CLI tools with minimal output formatting
- Infrastructure / DevOps tooling
- Backend-only services with no user-facing API responses

When skip conditions are detected, default the selection to "Skip" but still allow the user to choose Interactive.

### Choice

Ask the user: "How would you like to define product guidelines (voice, tone, UX principles, branding)?"
Options:
- **Interactive** -- Answer questions about brand voice and UX principles
- **Autogenerate** -- I'll generate sensible defaults based on the product type _(only if product.md exists)_
- **Skip** -- No product guidelines needed for this project

If **Skip**: write a minimal placeholder file:
```markdown
# Product Guidelines

No product guidelines defined. This project does not have user-facing content that requires voice, tone, or UX guidelines.
```
Continue to next step.

### Auto-inference Defaults

When autogenerating, use the product type from Step 6:

| Product Type | Default Voice | Default UX Principles |
|-------------|--------------|----------------------|
| Developer tool / library | Professional and direct | Zero-config defaults, progressive disclosure |
| Consumer web app | Friendly and approachable | Accessible by default, mobile-first |
| Enterprise / B2B | Formal and authoritative | Progressive disclosure, accessible by default |
| Internal tool | Professional and direct | Zero-config defaults |

### Interactive Questions

**Q1 -- Voice and tone:**

Ask the user: "What is the voice and tone for written content (UI copy, docs, error messages)?"
Options:
- **Professional and direct** -- Clear, concise, no fluff. Suitable for developer tools.
- **Friendly and approachable** -- Warm, conversational. Suitable for consumer apps.
- **Formal and authoritative** -- Precise, structured. Suitable for enterprise/compliance.
- **Playful and energetic** -- Fun, engaging. Suitable for consumer/gaming.
- **Let me describe** -- Type custom voice/tone guidelines

**Q2 -- UX principles:**

Ask the user: "What are the core UX principles?" (select all that apply)
Options:
- **Progressive disclosure** -- Show only what's needed; reveal complexity on demand
- **Zero-config defaults** -- Work out of the box; power users can customize
- **Accessible by default** -- WCAG AA minimum; keyboard navigable; screen reader support
- **Mobile-first** -- Design for small screens first, scale up
- **Let me describe** -- Type custom UX principles

**Q3 -- Branding:**

_Skip condition:_ Project has no UI (CLI tool, library, backend service). When skipped, write "No branding constraints -- project has no visual UI."

Ask the user: "Any branding or visual identity constraints? (color palette, typography, logo usage)"
Options:
- **No branding constraints** -- Skip; no visual identity rules
- **Let me describe** -- Type branding guidelines or link to a style guide

### Verification

Display `product-guidelines.md`. If the product is a developer tool, verify the voice section does not describe consumer-app tone (common autogenerate error).

---

## Workflow Configuration Interview (Step 10)

Generates `.maestro/context/workflow.md`. Source of truth for how `/maestro:implement` executes tasks.

### Choice

Ask the user: "How would you like to configure the workflow?"
Options:
- **Interactive** -- Answer questions about methodology and commit strategy
- **Autogenerate** -- Use recommended defaults (TDD, 80% coverage, per-task commits)

**When to recommend autogenerate:** Most projects should use the defaults. Only offer Interactive when the user has specific workflow requirements (legacy CI, unusual commit conventions, non-standard testing).

### Interactive Questions

**Q1 -- Methodology:**

Ask the user: "What development methodology should tasks follow?"
Options:
- **TDD (Recommended)** -- Write failing tests first, then implement. Red-Green-Refactor.
- **Ship-fast** -- Implement first, add tests after. Faster but less rigorous.
- **Custom** -- Define your own workflow

If **Custom**: Ask the user to describe their workflow in free text. Map it to the workflow template as closely as possible.

**Q2 -- Coverage target:**

Ask the user: "What test coverage target for new code?"

_Auto-inference:_ If a coverage config exists (jest `coverageThreshold`, pytest-cov config, `.nycrc`), extract the existing target and present as default.

Options:
- **80% (Recommended)** -- Good balance of coverage and velocity
- **90%** -- High coverage, slower velocity
- **60%** -- Basic coverage, maximum velocity
- **No target** -- Don't enforce coverage thresholds

**Q3 -- Commit frequency:**

Ask the user: "How often should implementation commit?"
Options:
- **Per-task (Recommended)** -- Atomic commit after each task completes. Fine-grained history.
- **Per-phase** -- Commit after each phase completes. Fewer, larger commits.

**Q4 -- Summary storage:**

Ask the user: "Where should task summaries be stored?"
Options:
- **Git notes (Recommended)** -- Attach detailed summaries as git notes on commits
- **Commit messages** -- Include full summary in the commit message body
- **Neither** -- No additional summaries beyond standard commit messages

### Development Commands

After the methodology questions, populate the development commands section of the workflow template.

**For brownfield projects:** Detect commands from `package.json` scripts, `Makefile` targets, or equivalent. Present for confirmation:

```
Detected commands:
  test:      bun test
  lint:      bun run lint
  build:     bun run build
  dev:       bun run dev
  format:    bun run format
  typecheck: bunx tsc --noEmit
```

Ask: "Are these correct?"

**For greenfield projects:** Populate defaults based on the tech stack from Step 7:

| Stack | test | lint | build | dev |
|-------|------|------|-------|-----|
| TypeScript + bun | `bun test` | `bunx biome check` | `bun run build` | `bun run dev` |
| Python + uv | `uv run pytest` | `uv run ruff check` | _n/a_ | `uv run python -m {module}` |
| Rust | `cargo test` | `cargo clippy` | `cargo build` | `cargo run` |
| Go | `go test ./...` | `golangci-lint run` | `go build` | `go run .` |

Ask: "I'll use these default commands. Any changes?"

### Verification

Display `workflow.md`. Critical checks:
- **Test command must work.** If the test command is wrong, `maestro:implement` will fail on every task.
- **Lint command must exist.** If there's no linter configured, note "No linter configured" rather than guessing.
- **Coverage command must match test framework.** `jest --coverage` vs `pytest --cov` vs `cargo tarpaulin`.

Write `workflow.md` using the template from `reference/workflow-template.md`.

---

## Code Style Guides (Step 12, Optional)

Available guides in `reference/styleguides/`:
- `python.md`, `typescript.md`, `javascript.md`, `go.md`, `general.md`, `cpp.md`, `csharp.md`, `dart.md`, `html-css.md`

### Pre-selection Logic

Match detected languages to available guides:

| Detected Language | Guide to Pre-select |
|-------------------|---------------------|
| TypeScript | `typescript.md` + `general.md` |
| JavaScript (no TS) | `javascript.md` + `general.md` |
| Python | `python.md` + `general.md` |
| Go | `go.md` + `general.md` |
| Rust | `general.md` (no Rust-specific guide) |
| C++ | `cpp.md` + `general.md` |
| C# | `csharp.md` + `general.md` |
| Dart/Flutter | `dart.md` + `general.md` |
| HTML/CSS (any web project) | `html-css.md` |

### Question

Ask the user: "Copy code style guides to your project? (based on detected stack: {languages})\n\nPre-selected: {matched guides}\nAlso available: {remaining guides}"
Options:
- **Yes, copy selected guides** -- Copy pre-selected style guides for detected languages
- **Yes, copy all guides** -- Copy all 9 style guides
- **Customize selection** -- Let me pick which guides to copy
- **Skip** -- No code style guides needed

If yes (any variant):
1. `mkdir -p .maestro/context/code_styleguides`
2. Copy selected guide files from `reference/styleguides/` to `.maestro/context/code_styleguides/`

---

## First Track (Step 14, Optional)

### Decision Guidance

Recommend "Yes" if:
- User mentioned a specific feature, bug, or task during the interview
- User seems ready to start building

Recommend "Skip" if:
- User is just exploring maestro
- User said they'll create tracks later
- Setup is being run as part of a larger onboarding process

### Question

Ask the user: "Would you like to create the first track now? A track represents a feature, bug fix, or other unit of work."
Options:
- **Yes, create a track** -- I'll describe a feature or task to start
- **Skip** -- I'll create tracks later with /maestro:new-track

### If Yes

Ask the user: "Describe the feature, bug fix, or task for the first track. Be as specific as you like."

Use the description to generate a track slug (kebab-case, max 5 words). Create `spec.md`, `plan.md`, `metadata.json`, `index.md` in `.maestro/tracks/{slug}/` and register in `.maestro/tracks.md`. See `reference/templates.md` for file formats.

### Verification

If a track was created:
1. Display the generated `spec.md` for confirmation.
2. Confirm the slug is readable and descriptive.
3. Confirm the track appears in `.maestro/tracks.md`.
