Read the local agent instructions in @AGENTS.md.

<!-- maestro:begin -->
Live maestro state is injected by hooks. Use `maestro status` for the current session view and `maestro ready` for available work.
Track work with `maestro work add|start|done`; method depth: `maestro recipe show work`.
If no harness hook fired, run `maestro hook record --event SessionStart` and read the brief from stdout.
Failed commands print a JSON error envelope on stderr and exit nonzero; the message names the next command to run.
<!-- maestro:end -->
