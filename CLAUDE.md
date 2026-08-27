Read the local agent instructions in @AGENTS.md.

<!-- maestro:begin -->
A session in this repository is its Lead; panes it opens with a dispatch are Peers; the room at ~/maestro is the Supervisor. Roles: `maestro recipe show slp`.
The repository's own `AGENTS.md` and `CLAUDE.md` text outside this block is its Workspace Protocol and may declare protected areas, hotspots, restart rules, and local verification; read it before taking work or opening a dispatch.
Live maestro state is injected by hooks. Use `maestro status` for the current session view and `maestro ready` for available work.
Track work with `maestro work add|start|done`; method depth: `maestro recipe show work`.
If no harness hook fired, run `maestro hook record --event SessionStart` and read the brief from stdout.
Failed commands print a JSON error envelope on stderr and exit nonzero; when the fix is mechanical, the message names the next command to run.
<!-- maestro:end -->
