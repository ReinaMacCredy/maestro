# Security policy

## Supported versions

Maestro is distributed from source and installed by fast-forwarding a checkout
of `main`. Only the current release on `main` is supported. Older tags receive
no backported fixes; upgrade with `maestro update`.

## Reporting a vulnerability

Report privately through GitHub's
[private vulnerability reporting](https://github.com/ReinaMacCredy/maestro/security/advisories/new)
on this repository. Do not open a public issue, pull request, or discussion for
a suspected vulnerability.

A report is most useful when it names the affected file and line, the exact
commands or repository state that reach the problem, and what an attacker gains.

Expect an acknowledgement within seven days. This is a single-maintainer
project with no paid support and no bounty; fixes land on `main` and are
announced in `CHANGELOG.md`.

## What is in scope

Maestro runs on a developer machine, reads and writes each repository's
`.maestro/` store, executes plugin code, and is invoked automatically by coding
agent harness hooks. In scope:

- Code execution reached without an explicit user action, in particular through
  repository-supplied content such as plugins, configuration, or store rows.
- Escaping a documented read-only boundary, including `MAESTRO_READ_ONLY=1` and
  any verb declared `mutates: false`.
- Reading or writing outside the current repository's `.maestro/` directory and
  the documented `~/.maestro` and `~/.local/bin` install paths.
- Corrupting or forging the durable record: work state, leases, decisions,
  dispatches, handbacks, or the event log, from outside the CLI contract.
- Weaknesses in `scripts/install.sh` and `maestro install` that let a third
  party influence what gets installed.

## What is out of scope

- Anything requiring an attacker who already has your shell or user account.
- The behavior of the coding agents Maestro coordinates, and of Herdr, Bun,
  Git, or any other external tool.
- Repository content you wrote yourself, or a plugin you explicitly trusted.
- Denial of service through ordinary resource use, such as a very large store.
