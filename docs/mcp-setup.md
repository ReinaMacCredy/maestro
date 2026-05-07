# MCP Setup

The `bun run install:local` script (and the equivalent step in published releases) configures supported agent runtimes automatically. This document covers the manual path and what the install does on your behalf.

## What `install:local` writes

After replacing `dist/maestro` on your `PATH`, `install:local` looks for these files and merges in an `mcpServers.maestro` entry pointing at the freshly installed binary:

| Runtime | Config file |
|---------|-------------|
| Claude Code | `~/.claude/mcp.json` |
| Codex | `~/.codex/mcp.json` |

The merge is non-destructive: existing servers and unrelated keys are preserved. Only the `command` and `args` of the maestro entry are updated. If the runtime's parent directory does not exist (`createIfMissing: false`), the install skips it entirely — it does not litter stub directories onto machines that do not have the runtime.

After install, restart your agent runtime so it picks up the new MCP server.

## Verifying the install

```bash
maestro mcp check
```

Reports:
- whether the maestro binary exists at the expected install path
- for each known runtime, whether `mcpServers.maestro` is configured and whether the entry's command matches the installed binary

`--json` switches the output to machine-readable form.

## Manual configuration

If your runtime is not on the auto-configure list, add an entry like this to your runtime's MCP config:

```json
{
  "mcpServers": {
    "maestro": {
      "command": "/absolute/path/to/maestro",
      "args": ["mcp", "serve"]
    }
  }
}
```

Use the absolute path to the installed binary (`command -v maestro` will resolve it). The Bun-compiled binary embeds its runtime, so no separate Node bundle or `node` command is required.

## Project scoping

The server walks up from its working directory looking for `.maestro/`. If your runtime launches the server from a directory above the maestro project, set `MAESTRO_PROJECT_ROOT` in the entry's `env` block:

```json
{
  "mcpServers": {
    "maestro": {
      "command": "/absolute/path/to/maestro",
      "args": ["mcp", "serve"],
      "env": { "MAESTRO_PROJECT_ROOT": "/absolute/path/to/project" }
    }
  }
}
```

## Troubleshooting

**`Not in a maestro project`** — the server could not find a `.maestro/` ancestor. Either run the runtime from inside a maestro project or set `MAESTRO_PROJECT_ROOT`.

**`maestro mcp check` shows `[stale]` for a runtime** — the runtime's config still references an older binary path. Re-run `bun run install:local` (or the release installer) to refresh the entry.

**Tools missing in the agent's tool list** — the runtime may need to be restarted after the config write. Check the runtime's logs for MCP startup errors; the maestro server logs to stderr, which the runtime usually captures.
