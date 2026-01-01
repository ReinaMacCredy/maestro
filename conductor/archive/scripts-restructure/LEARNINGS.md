# Learnings: Scripts Restructure

## Summary

Restructured Python scripts to match claudekit-skills pattern by relocating scripts to skill-specific `scripts/` directories with self-contained code and standardized CLI interfaces.

## Commands

- `uv run skills/conductor/scripts/artifact_query.py <query> --json` - JSON output for scripting
- `uv run skills/beads/scripts/track_assigner.py <beads.json> --json` - Generate track assignments from beads

## Gotchas

- Scripts use stdlib only (no external dependencies) - claudekit-skills pattern
- find_conductor_root() walks up from cwd to find conductor/ - works from any directory
- Underscore naming for Python files (artifact_query.py not artifact-query.py)

## Patterns

- **Self-Contained Scripts:** Inline shared lib functions into each script - no cross-script imports
- **CLI Scaffolding:** argparse + `--json` flag for structured output on all scripts
- **Skill Directory Structure:** SKILL.md + references/ + scripts/ (claudekit-skills pattern)
