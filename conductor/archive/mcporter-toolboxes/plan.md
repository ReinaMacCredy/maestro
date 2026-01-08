# Plan: MCPorter Toolboxes Integration

## Track Info

- **ID:** mcporter-toolboxes
- **Design:** [design.md](design.md)
- **Spec:** [spec.md](spec.md)

## Tasks

### Phase 1: Setup

- [x] **1.1** Create `toolboxes/` directory
- [x] **1.2** Create `mcporter.json` config with agent-mail server definition
- [x] **1.3** Create `README.md` with toolboxes overview

### Phase 2: Generate Agent Mail CLI

- [x] **2.1** Verify Agent Mail MCP server is running
- [x] **2.2** Run `npx mcporter generate-cli` for agent-mail
- [x] **2.3** Make generated CLI executable (`chmod +x`)
- [x] **2.4** Test CLI with `--help` and basic commands

### Phase 3: Documentation

- [x] **3.1** Create `maestro-core/references/toolboxes.md`
- [x] **3.2** Add toolboxes reference to maestro-core SKILL.md
- [x] **3.3** Create `orchestrator/references/agent-mail-cli.md` with usage examples

### Phase 4: Integration

- [x] **4.1** Update orchestrator skill to reference CLI as alternative to MCP
- [x] **4.2** Test full workflow: register agent, send message, fetch inbox

### Phase 5: Validation

- [x] **5.1** Verify all acceptance criteria from spec
- [x] **5.2** Update tracks.md with completed track

## File Scope

| Task | Files |
|------|-------|
| 1.1-1.3 | `toolboxes/*` |
| 2.1-2.4 | `toolboxes/agent-mail.*` |
| 3.1 | `skills/maestro-core/references/toolboxes.md` |
| 3.2 | `skills/maestro-core/SKILL.md` |
| 3.3 | `skills/orchestrator/references/agent-mail-cli.md` |
| 4.1 | `skills/orchestrator/SKILL.md` |
| 5.2 | `conductor/tracks.md` |

## Notes

- Agent Mail MCP must be running at `http://127.0.0.1:8765/mcp/` for generation
- If server unavailable, generation will fail - this is expected
- Token should be set in `AGENT_MAIL_TOKEN` env var before generation
