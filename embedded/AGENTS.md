# Resources Agent Notes

## OVERVIEW

`embedded/` contains shipped, extracted, or installed content that the Rust
binary embeds and writes into user repositories.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Harness protocol template | `harness/HARNESS.md` | Versioned frontmatter; changing it affects new init output. |
| Hook recorder script | `hooks/record.sh` | Version marker uses `# maestro:hook-version:`. |
| Hook event config | `hooks/events.yaml` | Shared hook event names and accepted events. |
| Shell wrappers | `shell/posix.sh`, `shell/fish.fish` | Shell integration resources. |
| Bundled skills | `skills/<name>/SKILL.md` | Directory packages; preserve nested resources when present. |

## CONVENTIONS

- If a shipped, version-gated resource changes, update its marker and the
  matching `(version, tree-hash)` row in `tests/resources_version_guard.rs`.
- Skills are directory packages. Keep `SKILL.md`, `references/`, `scripts/`,
  and `assets/` together when adding nested content.
- Runtime extraction policy lives in `src/domain/extraction/`,
  `src/domain/harness/`, `src/domain/skills/`, and `src/domain/install/`.

## ANTI-PATTERNS (THIS PROJECT)

- Do not change resource text without deciding whether existing user-owned
  installs should update automatically, only on force/apply, or never.
- Do not ship development-only `evals/` content unless a runtime skill needs it.
- Do not forget executable metadata for scripts that are installed or extracted.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [schemas/AGENTS.md](schemas/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
