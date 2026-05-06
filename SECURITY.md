# Security Policy

Maestro is a local-first CLI: it runs on a developer machine, persists state
on disk, and shells out to git, agent harnesses, and CI. Most of its trust
boundaries are local. The areas worth flagging:

## Trust boundaries

- **Filesystem reads/writes.** Maestro reads `.maestro/`, `.git/`, agent
  config files, and skill directories. It writes to `.maestro/`, `~/.maestro/`,
  agent skill roots, and (when `maestro install` runs) the agent harness
  config files at `~/.claude/`, `~/.codex/`, and `~/.hermes/`.
- **Subprocess execution.** Handoff launches start a fresh agent process
  (`codex`, `claude`, `hermes`) with arguments derived from the operator's
  prompt and the local mission state. The launched binary is whatever resolves
  on `PATH`.
- **Skill install sources.** `maestro skills install <source>` accepts local
  directories, Git URLs, GitHub shorthand, and HTTP archive URLs. Sources are
  fetched and unpacked as data, validated for `SKILL.md`, and copied into
  Maestro-managed storage. Bundled scripts inside a skill are never executed
  during install.
- **CI integration.** `maestro ci verify` reads `GITHUB_*` env, calls
  `gh api`, and posts a GitHub Check. The token is whatever the workflow
  exposes; Maestro never writes it to disk.

## What Maestro does not do

- It does not open inbound network sockets.
- It does not phone home for analytics.
- It does not execute skill scripts during install or sync.

## Reporting a vulnerability

If you believe you have found a security issue in Maestro, please report it
privately:

- Email: <reina@reinamaccredy.me>

Do not open a public issue with reproduction details until a fix is available.

Include in your report:

- `maestro --version`
- A minimal reproduction.
- The trust boundary involved (filesystem, subprocess, skill source, CI, etc.).
- Expected vs observed behavior.

You should receive an acknowledgement within a few business days. Coordinated
disclosure is preferred; we will work with you on a public advisory once a
patched release is out.

## Scope notes

- `~/.maestro/handoff/<id>/prompt.md` and `output.log` may contain anything
  the operator typed or the agent printed. They are local, gitignored, and
  not transmitted by Maestro itself, but treat them as you would any other
  local agent log.
- Evidence rows under `.maestro/evidence/` (gitignored) include command
  strings and exit codes. Do not record secrets in `--note` text or
  `--command` strings.
- Policy files under `.maestro/policies/` (committed) are intentionally
  reviewable: tightenings take effect immediately, loosenings soak for 30
  days. See [`docs/policy-format.md`](docs/policy-format.md).
