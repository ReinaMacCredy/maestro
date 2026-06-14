# Author loop.md and route it through SKILL.md and HARNESS

2026-06-12  TDD skipped: static skill reference content (markdown) with no testable runtime behavior; coverage is the re-recorded extraction/version guards (resources_version_guard 4/4, skills_extract 21/21 incl. new loop entry in the extract assertion).
2026-06-12  Regression caught by full suite: HARNESS.md bump to 1.12.0 broke src/domain/harness/extract.rs:134 which pins the frontmatter version literal a second time (beyond resources_version_guard). Fixed by updating the literal — the test enforces acknowledgement of harness edits, same contract as the guard table; softening it was not an option.
