---
name: release
description: "Automates version bumping, tagging, publishing, and GitHub release creation. Use when preparing and shipping a project release."
argument-hint: "<version|patch|minor|major> [--dry-run]"
disable-model-invocation: true
---

# Release — Automated Versioned Release Workflow

> Run a safe, project-agnostic release pipeline: preflight, version bump, commit, tag, optional push, optional publish, optional GitHub release.

## Arguments

- `<version>` — Explicit version (for example `1.2.3`) or bump type (`patch`, `minor`, `major`)
- `--dry-run` — Show what would happen without making changes

## Hard Rules

- **Project-agnostic**: Detect project type dynamically. Do not assume fixed file paths beyond standard manifests.
- **Stop on failing tests**: If preflight tests fail, STOP immediately.
- **Confirmation gates required**: Push, publish, and GitHub release MUST each require explicit user confirmation via `AskUserQuestion`.
- **No side effects in dry run**: `--dry-run` never edits files, commits, tags, pushes, publishes, or creates releases.

## Step 1: Preflight

### 1.1 Detect project type

Check for manifests in this order (first match wins as primary type; still collect all present manifests for version updates):

- `package.json` → npm/bun project
- `setup.py` or `pyproject.toml` → Python project
- `Cargo.toml` → Rust project
- `.claude-plugin/plugin.json` → Claude plugin

Use `Glob` to detect files. If none found, stop and report unsupported project type.

### 1.2 Read current version

Read the primary manifest and extract current version:

- `package.json` → `version`
- `pyproject.toml` → `[project].version` (or tool-specific version field if present)
- `setup.py` → `version=` value
- `Cargo.toml` → `[package].version`
- `.claude-plugin/plugin.json` → `version`

If current version cannot be determined, stop and report exactly which file failed parsing.

### 1.3 Find all version occurrences

Search for files that contain the exact current version string and are likely version-bearing project files.

Suggested search strategy:

1. Identify candidate files via `Glob` (manifests, docs, lock/config metadata).
2. Use `Grep` for the exact current version string.
3. Build a deduplicated file list to update.

Always include detected manifest files in the candidate list, even if formatting differs.

### 1.4 Run test suite

Run the first applicable project test command:

```bash
# JavaScript/TypeScript (prefer bun when available)
bun test || npm test

# Python
pytest || python -m pytest

# Rust
cargo test
```

Rules:
- Choose command based on detected project type.
- If command is missing, try fallback shown above.
- If all applicable test commands are unavailable, report SKIP with reason and ask whether to continue.
- If tests fail, STOP and report failure output. Do not continue.

## Step 2: Version Bump

### 2.1 Resolve target version

From `<version>` argument:

- Explicit semantic version (e.g., `1.2.3`) → use as-is
- `patch` → `X.Y.(Z+1)`
- `minor` → `X.(Y+1).0`
- `major` → `(X+1).0.0`

If argument is missing or invalid, stop and report valid forms.

### 2.2 Update all detected files

Update version values in every file from preflight version discovery:

- Manifest version fields
- Plugin metadata version fields
- Any additional release-relevant references discovered in Step 1.3

Use minimal edits only for exact old→new version changes.

### 2.3 Show diff

Before any commit/tag action, display a concise diff summary of updated files and the old/new version replacements for user review.

## Step 3: Commit

Stage only the files changed for this release bump, then commit:

```bash
git add <changed files>
git commit -m "chore(release): v<version>"
```

If commit fails, stop and report the error.

## Step 4: Tag

Create the release tag:

```bash
git tag v<version>
```

If tag already exists, stop and ask user whether to reuse, replace manually, or choose a new version.

## Step 5: Push (CONFIRMATION REQUIRED)

Ask for explicit confirmation:

```
AskUserQuestion(
  questions: [{
    question: "Push commit and tag to remote? This will push to origin/<branch> and tag v<version>.",
    header: "Push",
    options: [
      { label: "Yes, push", description: "Push commit and tag to origin" },
      { label: "Skip", description: "Skip push (you can push manually later)" }
    ],
    multiSelect: false
  }]
)
```

If confirmed:

```bash
git push origin <branch>
git push origin v<version>
```

If skipped, continue and mark push as skipped in summary.

## Step 6: Publish (CONFIRMATION REQUIRED)

Determine publish command based on detected project type and available tooling:

- npm/bun project: `npm publish` (or `bun publish` if project uses bun publish flow)
- Python project: `uv publish` (fallback to project-standard publish command if uv publish is not configured)
- Rust project: `cargo publish`
- Claude plugin project: if registry publish is not applicable, mark as skipped with reason

Ask confirmation:

```
AskUserQuestion(
  questions: [{
    question: "Publish package to registry?",
    header: "Publish",
    options: [
      { label: "Yes, publish", description: "Run the project's publish command" },
      { label: "Skip", description: "Skip publishing" }
    ],
    multiSelect: false
  }]
)
```

If confirmed, run the selected publish command. If skipped, continue.

## Step 7: GitHub Release (CONFIRMATION REQUIRED)

Ask confirmation:

```
AskUserQuestion(
  questions: [{
    question: "Create GitHub release for v<version>?",
    header: "Release",
    options: [
      { label: "Yes, create release", description: "Create GitHub release with auto-generated notes" },
      { label: "Skip", description: "Skip GitHub release" }
    ],
    multiSelect: false
  }]
)
```

If confirmed:

```bash
gh release create v<version> --title "v<version>" --generate-notes
```

Capture and report the release URL from command output.

## Dry Run Mode

If `--dry-run` is present, execute only analysis/reporting actions:

1. Run preflight (project detection, current version detection, version file discovery, tests)
2. Calculate and display target version
3. Show which files would be modified
4. Show the exact commands that would run for:
   - commit
   - tag
   - push
   - publish
   - GitHub release
5. Exit with no changes made

## Failure Handling

- Test failure in preflight → STOP
- Version parse failure → STOP
- File update mismatch (old version not found where expected) → STOP and report file
- Git commit/tag/push/publish/release failure → STOP and report command output

## Output Report

Always end with:

```markdown
## Release Summary

**Version**: v<version>
**Project type**: <type>
**Files updated**: <count>
**Tests**: PASS/FAIL/SKIP
**Pushed**: Yes/No/Skipped
**Published**: Yes/No/Skipped
**GitHub Release**: Yes/No/Skipped

Release URL: <gh release URL if created>
```

If dry run:
- Prefix summary with `DRY RUN`.
- Include `No changes were made.`
