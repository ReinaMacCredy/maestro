---
version: 1.0.0
---

# Code Playbook

Per-language code styleguides, surfaced on demand. The harness protocol
(`.maestro/harness/HARNESS.md`, `## Code style`) carries the universal
principles that apply everywhere. This folder carries the language-specific
detail.

## How to use this

When you are about to write or change code, read the one file that matches the
language you are editing, then follow it alongside the universal principles and
this repo's `AGENTS.md`. Do not load every file; load the one you need.

    editing Rust        -> rust.md
    editing Python      -> python.md
    editing Go          -> go.md
    editing TypeScript  -> typescript.md
    editing JavaScript  -> javascript.md
    editing C++         -> cpp.md
    editing C#          -> csharp.md
    editing Dart        -> dart.md
    editing HTML / CSS  -> html-css.md
    no language fits     -> general.md

Project-specific conventions always win over a general styleguide. When this
repo's `AGENTS.md` or an existing file's established style conflicts with a
guide here, follow the repo.

## Attribution

`cpp.md`, `csharp.md`, `dart.md`, `general.md`, `go.md`, `html-css.md`,
`javascript.md`, `python.md`, and `typescript.md` are vendored byte-verbatim
from the Gemini CLI Conductor extension
(https://github.com/gemini-cli-extensions/conductor,
`templates/code_styleguides`), licensed under the Apache License, Version 2.0
(https://www.apache.org/licenses/LICENSE-2.0). Each file retains its own
upstream source citation. `rust.md` is authored by Maestro (the Conductor set
ships no Rust guide).
