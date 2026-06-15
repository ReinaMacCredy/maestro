# Code Playbook

Per-language code styleguides, served on demand by `maestro playbook`. The
harness protocol (`.maestro/harness/HARNESS.md`, `## Code style`) carries the
universal principles that apply everywhere; these guides carry the
language-specific detail.

## How to use this

When you are about to write or change code, run `maestro playbook <lang>` for
the language you are editing, then follow it alongside the universal principles
and this repo's `AGENTS.md`. Read the one guide you need, not the rest. Run
`maestro playbook` with no language to list the available guides.

Project-specific conventions always win over a general styleguide. When this
repo's `AGENTS.md` or an existing file's established style conflicts with a
guide here, follow the repo.

## Attribution

The `cpp`, `csharp`, `dart`, `general`, `go`, `html-css`, `javascript`,
`python`, and `typescript` guides are vendored byte-verbatim from the Gemini
CLI Conductor extension (https://github.com/gemini-cli-extensions/conductor,
`templates/code_styleguides`), licensed under the Apache License, Version 2.0
(https://www.apache.org/licenses/LICENSE-2.0). Each guide retains its own
upstream source citation. The `rust` guide is authored by Maestro (the
Conductor set ships no Rust guide).
