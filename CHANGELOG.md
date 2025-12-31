# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [4.3.0] - 2025-12-31


### Added


- Add Impact Assessor agent documentation and MCP server configuration template.

## [4.2.0] - 2025-12-30


### Added


- Add auto-orchestration after filing beads (fb)
- Auto-orchestration after filing beads


### Changed


- Migrate agent coordination patterns and documentation from conductor to orchestrator skill.


### Documentation


- Update changelog
- Update CODEMAPS to reflect auto-orchestration in beads skill
- Add auto-orchestration to all documentation


### Fixed


- Add explicit orchestrator auto-loading instruction to conductor skill
- Update maestro-core routing to note ci/implement exception
- Address Gemini bot review comments on PR #21

## [4.1.0] - 2025-12-29


### Added


- Add orchestrator skill for multi-agent parallel execution
- Create mcp.json for project-specific configuration
- Enhance `/conductor-implement` to auto-route to orchestrator based on track assignments and agent mail availability.
- Add mcp.json to configure mcp-agent-mail server.

## [4.0.0] - 2025-12-29


### Added


- Replace LEDGER.md with HumanLayer-inspired handoff system
- Add `continuity.js` hook tests for `SessionStart`, `PreCompact`, `PostToolUse` and remove legacy handoff file and index format tests.
- Document new Maestro Core commands, detailed routing logic, and validation gates.


### Documentation


- Fix TUTORIAL.md state files table - replace LEDGER.md with handoffs
- Clarify Maestro Core's role and add prerequisite pattern for dependent skills.

## [3.9.0] - 2025-12-29


### Added


- Centralize command routing and intent mapping documentation from `conductor` to `maestro-core`.

## [3.8.0] - 2025-12-29


### Added


- Add 5 validation gates to Maestro lifecycle


### Changed


- Standardize validation gate documentation for consistent LEDGER updates and status reporting.

## [3.7.0] - 2025-12-29


### Added


- Replace grounding system with parallel research protocol

## [3.6.0] - 2025-12-29


### Added


- Session continuity automatic via workflow entry points
- Add `--keep` flag to prevent auto-archiving and remove interactive archive prompt.
- Implement PR label-driven version bumping in the release workflow, adjust commit-based fallback logic, and add a note to AGENTS.md.


### Changed


- Reorganize maestro-config block structure
- DRY Session Lifecycle - reference maestro-core instead of duplicating


### Documentation


- Update CODEMAPS with maestro-core skill


### Fixed


- Add maestro-core first-message instruction to maestro-config block
- Correct SemVer labels - breaking changes belong in major, not minor
- Address all PR review comments


### Complete


- Central orchestration skill with 5-level hierarchy

## [3.5.0] - 2025-12-28


### Added


- LEDGER.log format documentation
- Add inline grounding triggers at phase transitions


### Changed


- Move prompts from subagent-dev
- Move coordination from dispatching
- Move continuity to ledger/
- Move doc-sync references
- Extract TDD content to conductor
- Extract verification content to conductor
- Extract finishing-branch to conductor
- Move discipline rules to AGENTS.md
- Split skills into cycle/gates, gate/rollback, branch-options/cleanup as plan


### Documentation


- Sync documentation with skill integration


### Fixed


- Update writing-skills references to conductor
- Copy full SKILL.md files instead of extracting summaries
- Update broken continuity references to conductor/ledger
- Strip whitespace in shell commands per gemini-code-assist review
- Convert bold text to proper ### headings (MD036)

## [3.4.0] - 2025-12-28


### Added


- Add tiered grounding system with enforcement
- Refine grounding system by removing emojis from enforcement levels, detailing mandatory skip requirements, and updating grounding sources.


### Documentation


- Add grounding system to skills codemap

## [3.3.0] - 2025-12-28


### Added


- Make TDD checkpoints enabled by default and introduce `--no-tdd` flag to disable them.

## [3.2.0] - 2025-12-28


### Added


- Introduce halt and warning for missing conductor setup

## [3.1.0] - 2025-12-27


### Added


- Add doc-sync skill for automatic documentation updates
- Complete state-consolidation track


### Fixed


- Improve frontmatter field extraction to correctly handle values with spaces.

## [3.0.0] - 2025-12-27


### Added


- Add workflow state machine with smart suggestions and auto-archive
- Establish comprehensive GitHub project configuration with issue/PR templates, security policy, dependabot, and enhanced workflows.


### Fixed


- Improve shell script robustness and temp file safety
- Replace emoji warnings with text and update GitHub repository URLs in issue templates.


### Complete


- 11 learnings extracted, state machine and UX automation archived

## [2.2.0] - 2025-12-27


### Added


- Add session state preservation across sessions and compactions


### Changed


- Improve continuity hook ledger management with structured parsing and serialization, add Python script `lib` directory, and adjust CI/CD release trigger.
- Improve continuity hook ledger management with structured parsing and serialization, add Python script `lib` directory, and adjust CI/CD release trigger.
- Update Python type hints to use `Optional` for nullable types.


### Conductor


- Complete track continuity-integration_20251227
- Update overview.md with continuity skill

## [2.1.0] - 2025-12-27


### Added


- Implement language-adaptive communication, update agent details, and expand brainstorming techniques.
- Add new BMB and BMGD agents, update existing agent definitions, and refine documentation.
- Remove `party-mode-backup` content, introduce `.beads` data management system, and add new link validation scripts.


### Changed


- Standardize skill metadata by moving version and triggers into structured fields.


### Documentation


- Update changelog [skip ci]


### Complete


- 2 learnings extracted

## [2.0.0] - 2025-12-26


### Added


- Migrate to spec-compliant skills-only architecture


### Changed


- Move workflows/ to skills/*/references/
- Remove commands/, merge to skills/
- Complete spec-compliant migration, enhance link and anchor validation, and update skill and tutorial paths.
- Update Party Mode workflow path, clarify skill-local directory structure, and refine migration verification steps.


### Documentation


- Update all references for new architecture
- /conductor-finish spec-compliant-migration track


### Fixed


- Correct revisions.md relative path in workflows.md

## [1.11.0] - 2025-12-26


### Added


- Integrate routing and session lifecycle


### Documentation


- Update LIFECYCLE flow pipeline with Beads-Conductor integration
- Update TUTORIAL.md pipeline diagram with Beads-Conductor integration
- Update changelog [skip ci]


### Fixed


- Address PR #7 review comments


### Complete


- 5 learnings extracted, archived

## [1.10.0] - 2025-12-25


### Added


- Beads-Conductor Integration


### Documentation


- Update CODEMAPS with beads-conductor integration


### Fixed


- Improve lock file age calculation robustness by adding a fallback for the `stat` command.
- Improve `conductor-migrate-beads` script robustness and efficiency, and update documentation formatting and paths.


### Complete


- 9 learnings extracted, full integration documented

## [1.9.0] - 2025-12-25


### Added


- Add molecular chemistry features and extensive reference documentation to the beads skill.


### Changed


- Consolidate beads, file-beads, review-beads into unified skill + workflow


### Documentation


- Enhance workflow documentation with commit types, code review, session protocol, and verification steps.
- Update changelog [skip ci]
- Update beads skill reference files with new and reordered entries.


### Complete


- 4 learnings extracted

## [1.8.0] - 2025-12-24


### Added


- Document AI session handoff, add workflow validation system, and introduce create-plan skill.
- Integrate agent_mail MCP for multi-agent coordination


### Fixed


- Address PR review comments

## [1.7.0] - 2025-12-24


### Added


- Integrate codemaps into conductor workflow
- Move state file creation to phase 1 of newTrack workflow
- Add track validation system with centralized checks
- Consolidate refresh into finish workflow


### Documentation


- Regenerate after track completion


### Fixed


- Resolve all PR review comments from CodeRabbit and Gemini

## [1.6.0] - 2025-12-23


### Added


- Initial release of my-workflow skills v1.0.0
- Add various new skills and methodologies, including root cause tracing, testing patterns, and workflow definitions.
- Establish conductor workflow management system with commands, workflows, schemas, and code style guides, and update conductor skill definition
- Add version metadata to skill definitions, include `demo.yml` in gitignore, and update writing skills documentation.
- Introduce a detailed plan format for the plan executor and update skill references in subagent development.
- Add `ground` and `decompose-task` commands, introduce `codemaps` skill, and update workflow management skills.
- Add a comprehensive tutorial, a setup guide, and a new Claude code setup template, while updating the main README, a skill, and a command.
- Introduce parallel subagent dispatch for filing beads, add plan auto-archiving, and update tutorial path.
- Introduces multi-agent coordination capabilities with the Beads Village MCP server, enabling concurrent AI agent work on the same codebase by managing task claiming and file locking.
- Introduces multi-agent coordination capabilities with the Beads Village MCP server, enabling concurrent AI agent work on the same codebase by managing task claiming and file locking.
- Introduce doc-sync skill and remove deprecated memory search functionality.
- Rename project and plugin from `my-workflow` to `maestro` and add Beads Village installation to setup guide.
- Add MIT license and append new project link to README
- Replace `session-start` and `session-end` commands with a new `compact` command structure, including detailed scoring rubrics and a judge prompt.
- Revise workflow pipeline diagram to illustrate new two-session process.
- Add description and owner details to marketplace configuration
- Add Maestro Plugin global config template and adjust gitignore for docs and conductor plans
- Enhance installation instructions in README and add local template reference to global config.
- Add post-completion instructions and workflow prompts for beads, brainstorming, and conductor skills.
- Remove `systematic-debugging` and `execution-workflow` skill references and triggers, and simplify `git-worktrees` skill mention.
- Add auto-cleanup phase to doc-sync skill workflow and update commit logic
- Restructure design document storage to `conductor/design`, consolidate skills, and introduce an execution handoff pattern.
- Add detailed session protocol, categorized trigger phrases, and Beads CLI/multi-agent documentation to global config template.
- Introduce a dedicated `ds` skill for design sessions and update related documentation.
- Update plugin and marketplace versions to 1.1.0.
- Implement doc-sync auto-cleanup, archive old design documents, and update skill versions.
- Update Conductor workflow to process one epic at a time with refined `/conductor-implement` logic and task scoping.
- Enhance conductor workflow with user control, mandatory grounding, and clearer skill execution
- Enhance conductor workflow with user control, mandatory grounding, and clearer skill execution
- Add `fb` and `rb` commands for Beads issue management and update plugin version.
- Parallelize `file-beads` and `review-beads` subagent dispatch, and add cross-epic validation to `review-beads`.
- Add documentation for conductor implement (ci) and conductor setup (ct) commands
- Update `maestro` plugin version in `marketplace.json` and document its versioning in `AGENTS.md`.
- Introduce manual workflow guide, `refresh` and `revise` commands/workflows, and update conductor plugin configurations and documentation.
- Implement Double Diamond design process and Party Mode
- Update mermaid diagrams with BMAD v6-style workflow
- Merge /conductor-newtrack and fb into unified flow
- Rework tutorial workflow diagram and add new conductor, pipeline architecture, and command documentation.
- Add instruction to load TDD skill and implement conditional TDD workflow.
- Add GitHub Actions CI/CD for changelog and versioning
- Add /conductor-finish command with doc-sync integration


### Changed


- Restructure agent skills for plan execution and workflow, introduce hooks, and consolidate code review into a dedicated agent.
- Remove retro and spike workflows, updating conductor and tutorial documentation to reflect verification phase.
- Delete numerous workflow command definition files.
- Remove deprecated skills, update beads and other skill documentation, and add new conductor plans.
- Restructure Conductor commands and introduce new core configuration and plan files.
- Remove `execution-workflow` and `brainstorming` skills, add `conductor-design` command, and update related skill and conductor documentation.
- Rename `ds` skill to `design` and increment versions for multiple skills.
- Restructure beads skills to top-level and update plugin configuration
- Reorganize commands and add maintenance workflows


### Documentation


- Add K&V integration design plan
- Convert workflow pipeline diagram from plain text to Mermaid format.
- Update README with correct links and author attributions for conductor and beads projects.
- Update setup guide with global agent configuration, revised tool dependencies, and key triggers.
- Refine setup guide by clarifying global installation, updating skill lists, and reorganizing project initialization steps.
- Update README with new installation methods and add local reference path for global config template.
- Add Claude Code-specific documentation, update agent installation instructions, and refresh tutorial links.
- Enhance agent setup instructions, add dedicated `CLAUDE.md` documentation, and update command references.
- Refine task claiming instructions and clarify thread URL recording.
- Clarify the description for the `design` task type in the global config template.
- Update README and TUTORIAL with new epic completion flow
- Add complete workflow architecture diagram to README and TUTORIAL.
- Standardize formatting and improve readability across skill documentation files.
- Streamline setup guide, global configuration, and workflow documentation, including agent installation and diagram updates.
- Add link to pipeline architecture documentation to README.
- Simplify Maestro installation instructions in README.
- Remove superpowers plugin references, simplify setup flow
- Update README and SETUP_GUIDE, add install-codex script


### Fixed


- Update author field to object format
- Apply Gemini code review feedback
- Address Copilot PR review comments
- Use env block for COMMITS to prevent shell interpretation


### Complete


- 11 learnings extracted


### Conductor


- Sync context with codebase
- Update Double Diamond track spec and plan
- Update spec/plan for BMAD alignment


### Doc-sync


- Add conductor structure and handoff mechanism


