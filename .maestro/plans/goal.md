 # Full Scope Plan: Maestro Provider Registry, AgentSkills, and Hermes

  ## Summary

  Build a full provider layer for Maestro, not a narrow compatibility patch. The final system should support:

  - Runtime providers: codex, claude, hermes
  - Skill targets: Codex, Claude, Hermes, shared AgentSkills root
  - Skill sources: bundled Maestro skills, project skills, user/shared AgentSkills roots, local directories, Git URLs, HTTP archive URLs
  - Provider commands, diagnostics, install/sync/uninstall, handoff launch, session identity, and docs

  Phasing below is execution order only. The acceptance target is the whole provider system.

  ## Key Public Interfaces

  - Add provider commands:
      - maestro providers list [--json]
      - maestro providers doctor [provider] [--json]
      - maestro skills list [--scope project|user|shared|all] [--json]
      - maestro skills inspect <name> [--json]
      - maestro skills install <source> [--scope user|project|shared] [--targets all|codex,claude,hermes,agentskills]
      - maestro skills remove <name> [--scope user|project|shared]
      - maestro skills sync [--targets ...]
  - Extend existing commands:
      - maestro install, maestro update --agents-only, and maestro uninstall --agents-only sync all provider skill targets.
      - maestro handoff --agent <codex|claude|hermes>.
  - Define canonical roots:
      - Maestro bundled source of truth: ~/.maestro/skills/
      - Maestro external managed skills: ~/.maestro/external-skills/
      - AgentSkills shared root: ~/.agents/skills/
      - Hermes root: ~/.hermes/skills/maestro/<skill-name>/
      - Codex root: $CODEX_HOME/skills/ or ~/.codex/skills/
      - Claude root: ~/.claude/skills/

  ## Implementation Changes

  - Create a provider registry under infra with:
      - ProviderId = codex | claude | hermes | agentskills
      - runtime capability, skill-target capability, detection rules, config path, skills root, layout, and handoff launcher metadata.
      - AgentSkills is a skill-source/shared-root provider, not a handoff runtime.
  - Refactor existing SUPPORTED_AGENTS install code to consume the registry while preserving JSON result shape.
  - Add a skills feature for AgentSkills-compatible discovery, parsing, validation, install, remove, inspect, and sync.
      - Parse SKILL.md frontmatter with existing yaml.
      - Required fields: name, description.
      - Preserve unknown metadata.
      - Deterministic precedence: project .maestro/skills > project .agents/skills > repo built-ins > user/shared roots > provider roots.
      - Collision diagnostics are warnings, not silent shadowing.
  - External skill install:
      - Accept local directory, Git URL, GitHub shorthand, and HTTP zip/tar archive URL.
      - Copy validated skill directories into Maestro-managed roots.
      - Never execute bundled scripts during install.
      - Write a managed manifest with source, resolved commit/hash when available, file hashes, installed target roots, and installed timestamp.
      - Do not implement marketplace slug lookup unless a stable documented AgentSkills registry API is found during implementation. The registry adapter boundary should
        exist, but undocumented slug resolution must fail with a clear message.
  - Hermes runtime:
      - Add HermesHandoffLaunchAdapter.
      - Command shape: hermes chat --quiet --yolo --toolsets terminal,skills --source maestro -q <prompt>.
      - If a model is explicitly provided, pass --model <model>.
      - If a Hermes provider is explicitly added later, pass --provider <provider> only through a dedicated Hermes-specific option, not by overloading --agent.
      - Add launch env support to runLoggedCommand, then set MAESTRO_AGENT=hermes and MAESTRO_SESSION_ID=<handoff-id> for Hermes child processes.
      - Extend session detection to read MAESTRO_AGENT and MAESTRO_SESSION_ID before Codex/Claude-specific detection.
  - Hermes config:
      - Add idempotent YAML mutation for ~/.hermes/config.yaml.
      - Ensure skills.external_dirs contains ~/.agents/skills.
      - Preserve comments only if the current YAML writer can do so safely; otherwise create a timestamped backup before rewriting.
  - Docs:
      - Document provider concepts, command examples, install roots, Hermes setup expectations, AgentSkills source support, and security model.
      - Update README/provider references and handoff examples.

  ## Tests and Verification

  - Unit tests:
      - provider registry path resolution and detection
      - AgentSkills parser, malformed YAML, required-field validation, metadata preservation
      - skill precedence and collision diagnostics
      - install/sync/uninstall preserving foreign skill directories
      - Hermes target detection and config.yaml external-dir mutation
      - Hermes handoff command/env construction
      - session detection via MAESTRO_AGENT and MAESTRO_SESSION_ID
  - Integration/e2e:
      - maestro install --json in isolated temp home with fake Codex, Claude, Hermes configs
      - maestro skills install from local dir and fake Git repo
      - compiled handoff e2e with fake hermes binary
      - uninstall removes only Maestro-managed links/manifests
  - Required final checks:
      - bun run build
      - bun run check:boundaries
      - bun run check:skills
      - bun run check:bundled-skills
      - bun test --path-ignore-patterns 'apps/desktop/**'
      - focused compiled CLI smokes for provider and handoff commands

  ## Assumptions and Defaults

  - Full scope includes remote source support through path, Git, GitHub shorthand, and HTTP archive URLs. It does not invent an undocumented agentskills.io marketplace slug
    API.
  - No new production dependency unless archive extraction cannot be implemented safely with existing runtime tools.
  - Existing Codex and Claude behavior must remain backward compatible.
  - Current unrelated dirty files must be preserved.
  - Repo-tracked behavior changes require a CLI patch version bump.
  - Before implementation edits, run GitNexus impact analysis for touched symbols if the tool is available.

  ## Proposed Contract

  proposed_contract:
    allowed_files:
      - "src/infra/**"
      - "src/features/handoff/**"
      - "src/features/session/**"
      - "src/features/skills/**"
      - "src/shared/**"
      - "tests/unit/infra/**"
      - "tests/unit/features/handoff/**"
      - "tests/unit/features/session/**"
      - "tests/unit/features/skills/**"
      - "tests/integration/**"
      - "tests/e2e/**"
      - "README.md"
      - "docs/**"
      - "package.json"
    forbidden_paths:
      - "bun.lock"
      - ".github/workflows/**"
      - "src/infra/domain/built-in-skill-templates.ts"
      - "src/infra/domain/bundled-skill-templates.ts"
      - ".maestro/policies/**"
    risk_class: high
    amendment_budget:
      max_amendments: 4
      max_paths_per_amendment: 8
      forbidden_amendment_paths:
        - "bun.lock"
        - ".github/workflows/**"
        - ".maestro/policies/**"

  Grounding sources:

  - https://agentskills.io/home
  - https://agentskills.io/specification
  - https://agentskills.io/client-implementation/adding-skills-support
  - https://hermes-agent.nousresearch.com/docs/user-guide/features/skills
  - https://hermes-agent.nousresearch.com/docs/reference/cli-commands