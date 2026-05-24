# init.sh template

`maestro setup` emits a small, project-owned `init.sh` at the repo root. It is
the cold-start trigger: one shell call runs the doctor health gate, then prints
the status snapshot, so an agent (human or otherwise) can resume work in one
file read.

## Emitted body

```bash
#!/usr/bin/env bash
# Project init -- emitted once by `maestro setup` and never overwritten.
# Edit freely; Maestro will not touch this file again unless you delete it.
set -euo pipefail

# Health gate -- exits non-zero if .maestro/ scaffold is broken.
maestro doctor

# Cold-start view -- one-screen resume snapshot.
maestro status
```

Mode after emission: `0755` on non-Windows platforms.

## Emit-once contract

- `maestro setup` writes `init.sh` only when it does not already exist.
- On reruns, existing content is preserved verbatim.
- The file is meant to be edited. Add project-specific preflight steps
  (dependency installs, env wiring, secrets check) above or below the two
  Maestro calls.

## How to regenerate

```bash
rm init.sh
maestro setup
```

This is the only supported regeneration path. There is no `--force-init` flag;
deleting the file is the explicit user signal that the template should be
re-emitted.

`maestro setup --reset-templates` is the global escape hatch for restoring any
shipped template, and it will also rewrite `init.sh`. Use it deliberately:
local edits are lost.

## Doctor pairing

`maestro doctor` includes an `init-script` dimension that checks for `init.sh`
at the repo root and verifies the execute bit is set. After a fresh
`maestro setup`, that dimension flips from `warn` to `ok` without further
action.

See `docs/cli-reference.md` for the full `doctor` verb shape.
