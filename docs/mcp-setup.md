# MCP Setup

`maestro install` (and the equivalent step in `bun run release:local`) configures supported agent runtimes automatically. This document covers the auto-configure flow, the manual path, and how to verify the result.

## Canonical config files

The MCP entry lands in the file the runtime actually reads:

| Runtime | Config file | Format |
|---------|-------------|--------|
| Claude Code (user scope) | `~/.claude.json` | JSON, top-level `mcpServers.maestro` |
| Codex | `~/.codex/config.toml` | TOML, `[mcp_servers.maestro]` table |

The installed binary path is platform-specific: `~/.local/bin/maestro` on macOS and Linux, `%LOCALAPPDATA%\maestro\maestro.exe` on Windows.

## What the auto-configure step does

1. Detects each runtime by looking for its CLI on `PATH` (`claude` for Claude Code, `codex` for Codex). A runtime that isn't on `PATH` is skipped — the install does not write configs for runtimes that aren't installed.
2. Reads the existing entry from the canonical file (JSON parse for Claude, table scan for Codex). If the entry already points at the freshly installed binary with the right args, the step is a no-op.
3. Otherwise, shells out to the runtime's own CLI to register the entry:
   - Claude Code: `claude mcp add maestro -s user -- /abs/path/to/maestro mcp serve` (preceded by `claude mcp remove maestro -s user` when overwriting a stale entry).
   - Codex: `codex mcp add maestro -- /abs/path/to/maestro mcp serve` (preceded by `codex mcp remove maestro` when overwriting).

Shelling out to the agent CLI is preferred over direct file edits: it handles serialization, atomic writes, and edge cases the same way the runtime expects, and avoids touching `~/.claude.json` (which holds the user's full Claude Code session state) more than necessary.

## Verifying the install

```bash
maestro mcp check
maestro mcp check --json
```

Reports:

- whether the maestro binary exists at the expected install path (`[ok]` / `[!!]`)
- per runtime, whether `mcpServers.maestro` (or `[mcp_servers.maestro]`) is configured and whether the entry's `command` matches the installed binary path:
  - `[ok]` — configured and current
  - `[stale]` — configured but pointing at a different binary path
  - `not configured` — no entry in the canonical file

Exit code is `1` when the binary is missing, `0` otherwise.

## Manual configuration

If your runtime is not on the auto-configure list, or the agent CLI was not on `PATH` at install time, add the entry directly.

### Claude Code (`~/.claude.json`)

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

`claude mcp add maestro -s user -- /absolute/path/to/maestro mcp serve` writes the same entry through the official CLI.

### Codex (`~/.codex/config.toml`)

```toml
[mcp_servers.maestro]
command = "/absolute/path/to/maestro"
args = ["mcp", "serve"]
```

`codex mcp add maestro -- /absolute/path/to/maestro mcp serve` writes the same entry through the official CLI.

Use the absolute path to the installed binary (`command -v maestro` will resolve it). The Bun-compiled binary embeds its runtime, so no separate Node bundle or `node` command is required.

## Project scoping

The server walks up from its working directory looking for `.maestro/`. If the runtime launches the server from a directory above the maestro project, set `MAESTRO_PROJECT_ROOT` in the entry's `env` block:

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

```toml
[mcp_servers.maestro]
command = "/absolute/path/to/maestro"
args = ["mcp", "serve"]
env = { MAESTRO_PROJECT_ROOT = "/absolute/path/to/project" }
```

## Troubleshooting

**`Not in a maestro project`** — the server could not find a `.maestro/` ancestor. Either run the runtime from inside a maestro project or set `MAESTRO_PROJECT_ROOT` in the entry's `env`.

**`maestro mcp check` shows `[stale]`** — the canonical config still references an older binary path. Re-run `bun run release:local` (or the release installer) to refresh the entry. The auto-configure step is idempotent; a `[stale]` entry is rewritten in place.

**`maestro mcp check` shows `not configured` for a runtime whose CLI is on `PATH`** — the agent CLI errored during the original `mcp add` call. Run the equivalent `claude mcp add` / `codex mcp add` invocation from the manual section above and inspect the output.

**Tools missing in the agent's tool list** — the runtime needs to be restarted after the config write. Check the runtime's logs for MCP startup errors; the maestro server logs to stderr, which the runtime usually captures.

**Maestro shows up under the runtime's project-scope MCPs but not user-scope** — an older install (≤ 0.75.0) wrote project-local `.mcp.json` files. Remove any committed `.mcp.json` and re-run the install; the user-scope entry is what `maestro mcp check` validates.

**Leftover files from old installs** — installs from ≤ 0.75.0 wrote to `~/.claude/mcp.json` and `~/.codex/mcp.json`. Neither is read by the agent runtimes today. After the next install, the script prints a one-time hint listing any leftover files; remove them with the suggested `rm` command.

**`disabledMcpServers`** — Claude Code keeps a per-project `disabledMcpServers` list in `~/.claude.json`. If you previously disabled `maestro` for a project, the entry is still configured but the tools are hidden. Remove `"maestro"` from `projects["<abs-path>"].disabledMcpServers` (or run `claude mcp` from that project to re-enable).
