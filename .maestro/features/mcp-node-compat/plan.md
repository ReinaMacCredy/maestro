# MCP Node.js Compatibility Fix

## Discovery

The MCP server bundle (`dist/server.bundle.mjs`) runs under Node.js via `start.mjs`, but uses `import.meta.dir` -- a Bun-only API -- in three files: `infra/toolbox/loader.ts`, `infra/toolbox/agents/loader.ts`, and `infra/toolbox/agents/registry.ts`. Node.js returns `undefined` for `import.meta.dir`, causing `path.join(undefined)` to throw `TypeError: ERR_INVALID_ARG_TYPE`. This blocks all MCP tool usage in Claude Code since the server crashes on startup. Root cause confirmed via debug log and manual `node start.mjs` reproduction.

## Non-Goals

- Fixing CLI-only `import.meta.dir` usages (toolbox add/create/remove/test commands) -- these run under Bun
- Changing the build target from Node to Bun
- Removing filesystem scanning in dev mode

## Ghost Diffs

- No changes to domain/ layer
- No changes to MCP tool handlers
- No changes to test files (existing tests continue to pass)

## Tasks

### 1. embed-tool-manifests
Generate `manifests.generated.ts` from 9 tool manifest JSON files at build time. Update `scanBuiltInManifests()` to return the embedded array.

### 2. embed-agent-data
Generate `agent-data.generated.ts` from 4 agent manifests, 3 guidance docs, 3 protocol files at build time. Update `scanAgentTools()`, `getGuidance()`, and `assembleProtocol()` to read from embedded data.

### 3. verify-mcp-server
Verify MCP server starts under Node.js, responds to initialize + tools/list, and all 26 tools are registered. Verify CLI still works. Run full test suite.
