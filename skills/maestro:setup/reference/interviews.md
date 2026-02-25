# Setup Interview Questions

Each interview step starts with an **Interactive vs Autogenerate** choice. If Autogenerate: use codebase inferences from the maturity detection scan, skip detailed questions, write the file directly.

---

## Product Definition Interview

Generates `.maestro/context/product.md`.

Ask the user: "How would you like to provide the product definition?"
Options:
- **Interactive** -- Answer questions step by step
- **Autogenerate** -- I'll infer everything from the codebase analysis

If **Interactive**, ask sequentially (limit 3 questions max):

**Q1 -- Project purpose:**

Ask the user: "What does this project do? (one sentence)"
Options:
- **{inferred purpose}** -- Based on README/package analysis
- **Let me describe it** -- Type your own description

For greenfield or when no inference is available, provide only "Let me describe it".

**Q2 -- Target users:**

Ask the user: "Who are the primary users?"
Options:
- **Developers** -- Library, CLI tool, or developer-facing API
- **End users** -- Web app, mobile app, or consumer-facing product
- **Internal team** -- Internal tool, admin dashboard, or ops tooling

**Q3 -- Key features** (skip if brownfield with clear README):

Ask the user: "What are the 2-3 most important features or capabilities?"
Options:
- **Auto-generate from analysis** -- I'll infer from the codebase
- **Let me list them** -- Type your own list

---

## Tech Stack Interview

Generates `.maestro/context/tech-stack.md`.

Ask the user: "How would you like to provide the tech stack?"
Options:
- **Interactive** -- Review and confirm the detected stack or enter it manually
- **Autogenerate** -- I'll infer the full tech stack from config files

If **Interactive**:

**For Brownfield**: Present inferred stack for confirmation:

Ask the user: "Is this your tech stack?\n\n{inferred stack summary}"
Options:
- **Yes, correct** -- Use the detected tech stack
- **Needs changes** -- Let me correct or add to it

**For Greenfield**: Ask directly:

Ask the user: "What tech stack will this project use? (languages, frameworks, database, etc.)"

---

## Coding Guidelines Interview

Generates `.maestro/context/guidelines.md`.

Ask the user: "How would you like to define coding guidelines?"
Options:
- **Interactive** -- Select from common principles and conventions
- **Autogenerate** -- I'll infer from CLAUDE.md, linter configs, and conventions

If **Interactive**:

Ask the user: "Any specific coding guidelines or principles for this project?" (select all that apply)
Options:
- **TDD-first** -- Test-driven development, high coverage
- **Move fast** -- Ship quickly, iterate later
- **Security-first** -- Input validation, audit logging, secure defaults
- **Accessibility-first** -- WCAG compliance, semantic HTML, screen reader support
- **Let me describe** -- Type custom guidelines

---

## Product Guidelines Interview

Generates `.maestro/context/product-guidelines.md`.

Ask the user: "How would you like to define product guidelines (voice, tone, UX principles, branding)?"
Options:
- **Interactive** -- Answer questions about brand voice and UX principles
- **Autogenerate** -- I'll generate sensible defaults based on the product type
- **Skip** -- No product guidelines needed for this project

If **Skip**: write a minimal placeholder file and continue.

If **Interactive**:

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

**Q3 -- Branding** (skip if no UI):

Ask the user: "Any branding or visual identity constraints? (color palette, typography, logo usage)"
Options:
- **No branding constraints** -- Skip; no visual identity rules
- **Let me describe** -- Type branding guidelines or link to a style guide

---

## Workflow Configuration Interview

Generates `.maestro/context/workflow.md`. Source of truth for how `/maestro:implement` executes tasks.

Ask the user: "How would you like to configure the workflow?"
Options:
- **Interactive** -- Answer questions about methodology and commit strategy
- **Autogenerate** -- Use recommended defaults (TDD, 80% coverage, per-task commits)

If **Interactive**:

**Q1 -- Methodology:**

Ask the user: "What development methodology should tasks follow?"
Options:
- **TDD (Recommended)** -- Write failing tests first, then implement. Red-Green-Refactor.
- **Ship-fast** -- Implement first, add tests after. Faster but less rigorous.
- **Custom** -- Define your own workflow

**Q2 -- Coverage target:**

Ask the user: "What test coverage target for new code?"
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

Write `workflow.md` using the template from `reference/workflow-template.md`.

---

## Code Style Guides (Optional)

Available guides in `reference/styleguides/`:
- `python.md`, `typescript.md`, `javascript.md`, `go.md`, `general.md`, `cpp.md`, `csharp.md`, `dart.md`, `html-css.md`

Ask the user: "Copy code style guides to your project? (based on detected stack: {languages})\n\nAvailable guides: python, typescript, javascript, go, general, cpp, csharp, dart, html-css"
Options:
- **Yes, copy relevant guides** -- Copy style guides for detected languages to .maestro/context/code_styleguides/
- **Yes, copy all guides** -- Copy all 9 style guides
- **Skip** -- No code style guides needed

If yes:
1. `mkdir -p .maestro/context/code_styleguides`
2. Copy relevant (or all) guide files from `reference/styleguides/` to `.maestro/context/code_styleguides/`

---

## First Track (Optional)

Ask the user: "Would you like to create the first track now? A track represents a feature, bug fix, or other unit of work."
Options:
- **Yes, create a track** -- I'll describe a feature or task to start
- **Skip** -- I'll create tracks later with /maestro:new-track

If **Yes**:

Ask the user: "Describe the feature, bug fix, or task for the first track. Be as specific as you like."

Use the description to generate a track slug (kebab-case, max 5 words). Create `spec.md`, `plan.md`, `metadata.json`, `index.md` in `.maestro/tracks/{slug}/` and register in `.maestro/tracks.md`. See `reference/templates.md` for file formats.
