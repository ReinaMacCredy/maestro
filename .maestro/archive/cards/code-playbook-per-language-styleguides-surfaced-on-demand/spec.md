# Code playbook: per-language styleguides surfaced on-demand

## Current state

HARNESS.md (v1.14.0) carries a ## Code style section with 7 language-neutral bullets, shipped by the archived feature code-playbook-styleguides-surfaced-to-agents. There is no per-language guidance. The version-gated extraction core (src/domain/extraction/) already extracts the hook script and harness protocol to .maestro/ via a folder gate keyed on an anchor file's version; the harness planner (src/domain/harness/extract.rs) governs HARNESS.md + RECOVERY.md from one HARNESS.md version. include_dir is already a dependency. This feature reuses that core to ship a playbook folder.

## Problem

