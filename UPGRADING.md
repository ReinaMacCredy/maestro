# Upgrading maestro

Maestro is a big-bang release tool. There is no parallel binary, no aliasing
of legacy verbs, and no automated migration shim.

If you are not ready to upgrade across a release that ships breaking changes,
pin to the prior tag and upgrade out-of-band. See [`CHANGELOG.md`](./CHANGELOG.md)
for per-release notes.

## What changes between releases

Per-release breaking changes are listed in `CHANGELOG.md` under the affected
version. Common surfaces:

- **CLI verbs.** Verbs may be renamed, removed, or merged. The current set
  is documented in [`docs/cli-reference.md`](./docs/cli-reference.md).
- **`.maestro/` directory layout.** The canonical layout is scaffolded by
  `maestro setup`. Audit drift with `maestro setup check`.
- **MCP tool surface.** Available tools are listed in [`docs/mcp-server.md`](./docs/mcp-server.md).
- **Skill bundle.** Shipped agent skills are listed in
  [`skills/bundled/`](./skills/bundled/).

## Upgrading an existing project

1. Install the new release binary.
2. Run `maestro setup` from the project root. The setup state machine is
   idempotent and reconciles the directory layout, skill drops, and
   project config.
3. Run `maestro setup check` to confirm the layout matches the current
   release's expectations.
4. Read the relevant `CHANGELOG.md` entry for any manual follow-ups (e.g.,
   workflow file regeneration, policy edits).

## If something breaks

1. **`setup` reports missing or stale entries.** Run `maestro setup` to
   reconcile, then `maestro setup check` to confirm.
2. **A verb you depended on is gone.** Check `CHANGELOG.md` for the
   release that removed it.
3. **You need to roll back.** Pin to the prior release tag.

## Reference

- Decision register: [`docs/adr/`](./docs/adr/)
- CLI reference: [`docs/cli-reference.md`](./docs/cli-reference.md)
- Per-release notes: [`CHANGELOG.md`](./CHANGELOG.md)
