# Mission Control TUI Reference

`maestro mission-control` is a read-only dashboard. It has two modes: interactive (full TUI, for humans) and preview (text frames to stdout, for agents).

## Interactive

```bash
maestro mission-control
```

Launches the full TUI. Use arrow keys to navigate, `q` to quit. Not agent-consumable.

## Preview mode (agent-friendly)

```bash
maestro mission-control --preview [screen] [--size WxH] [--format plain|ansi]
maestro mission-control --preview all [--size WxH] [--format plain|ansi]
maestro mission-control --render-check [--size WxH]
maestro mission-control --json [--size WxH]
```

### Flags

| Flag | Purpose |
|---|---|
| `--preview [screen]` | Render a single frame to stdout and exit. Optional positional picks a specific screen. |
| `--preview all` | Render every screen sequentially. |
| `--size WxH` | Set the virtual terminal size. Standard: `120x40`. |
| `--format plain` | Plain-text frame. Default when stdout is not a TTY. |
| `--format ansi` | ANSI-styled output. Default when stdout is a TTY. |
| `--render-check` | Validate every preview screen; prints a JSON report. Non-zero exit on failure. |
| `--json` | Print a structured snapshot instead of a rendered frame. Mutually exclusive with `--preview`. |
| `--mission <id>` | Override auto-detected mission. |
| `--feature <id>` | Select a feature for `dashboard`, `features`, or `dependencies` previews. |

### Screen names

`dashboard`, `features`, `dependencies`, `config`, `memory`, `graph`, `agents`, `dispatch`, `events`, `tasks`, `timeline`, `principles`, `help`. Aliases: `feat`, `cfg`, `deps`, `mem`.

### Conventional invocations

Default screen, plain text:
```bash
maestro mission-control --preview --size 120x40 --format plain
```

All screens, plain text (full diagnostic dump):
```bash
maestro mission-control --preview all --size 120x40 --format plain
```

Specific screen:
```bash
maestro mission-control --preview features --size 120x40 --format plain
```

JSON snapshot (when the agent needs structured state, not a rendered frame):
```bash
maestro mission-control --json --size 120x40
```

Render health check:
```bash
maestro mission-control --render-check --size 120x40
```

## Common mistakes

- **`--format json` is not valid.** Use the top-level `--json` flag for structured output. `--format` only accepts `plain` or `ansi` and only applies when `--preview` is set.
- **`--json` and `--preview` can not be combined.** The CLI rejects that pair with an error and hints at which to use.
- **`--screen <name>` is not a flag.** The screen name is the optional positional argument to `--preview` (e.g., `--preview features`).

## Read-only contract

Preview, JSON, and render-check paths are guaranteed read-only. They build the mission-control state via `buildSnapshot()` / `buildHomeSnapshot()` and render without mutating anything on disk.

If preview output appears to show stale data, the cause is upstream state, not the preview path.

## Troubleshooting

- **Preview returns empty frame.** The current cwd may not be inside a maestro project. Check that `.maestro/` exists in cwd or an ancestor.
- **Render-check exits non-zero.** Rendering failure; inspect stderr for the specific screen and error.
- **Frame cuts off.** Increase `--size`. Agent-friendly default is `120x40`; wide dashboards may need `180x60`.
