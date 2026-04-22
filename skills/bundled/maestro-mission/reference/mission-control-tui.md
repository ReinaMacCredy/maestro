# Mission Control TUI Reference

`maestro mission-control` is a read-only dashboard. It has two modes: interactive (full TUI, for humans) and preview (text frames to stdout, for agents).

## Interactive

```bash
maestro mission-control
```

Launches the full TUI. Use arrow keys to navigate, `q` to quit. Not agent-consumable.

## Preview mode (agent-friendly)

```bash
maestro mission-control --preview [--size WxH] [--format plain|json] [--screen <name>]
maestro mission-control --preview all [--size WxH] [--format plain|json]
maestro mission-control --render-check [--size WxH]
```

### Flags

| Flag | Purpose |
|---|---|
| `--preview` | Render a single frame to stdout and exit. |
| `--preview all` | Render every screen sequentially. |
| `--size WxH` | Set the virtual terminal size. Standard: `120x40`. |
| `--format plain` | Plain-text frame suitable for reading by agents. |
| `--format json` | Structured snapshot of the screen state. |
| `--render-check` | Validate rendering without printing. Non-zero exit on failure. |
| `--screen <name>` | Render a specific screen (home, missions, tasks, etc.). |

### Conventional invocations

Default screen, plain text:
```bash
maestro mission-control --preview --size 120x40 --format plain
```

All screens, plain text (use for full diagnostic dumps):
```bash
maestro mission-control --preview all --size 120x40 --format plain
```

JSON snapshot (use when the agent needs structured state, not a rendered frame):
```bash
maestro mission-control --preview --size 120x40 --format json
```

Render health check (use in CI or before trusting preview output):
```bash
maestro mission-control --render-check --size 120x40
```

## Read-only contract

Preview, JSON, and render-check paths are guaranteed read-only. They build the mission-control state via `buildSnapshot()` / `buildHomeSnapshot()` and render without mutating anything on disk.

If preview output appears to show stale data, the cause is upstream state, not the preview path.

## Troubleshooting

- **Preview returns empty frame.** The current cwd may not be inside a maestro project. Check that `.maestro/` exists in cwd or an ancestor.
- **Render-check exits non-zero.** Rendering failure; inspect stderr for the specific screen and error.
- **Frame cuts off.** Increase `--size`. Agent-friendly default is `120x40`; wide dashboards may need `180x60`.
