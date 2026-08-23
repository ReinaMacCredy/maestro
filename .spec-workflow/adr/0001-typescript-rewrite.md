# Rewrite maestro in TypeScript on bun, distributed as a bun shim

The Rust implementation (~309k lines) made iteration expensive: slow builds, no
hot reload, and none of Rust's strengths (raw performance, memory control) matter
for a local-first CLI orchestrator. We rebuild from scratch in TypeScript running
on bun, distributed as source plus a `#!/usr/bin/env bun` shim at
`~/.local/bin/maestro` — no compile step, every invocation runs current source.
The new build takes the `maestro` name immediately (user decision, accepted
knowingly); the last Rust binary is kept installed as `maestro-legacy` as the
rollback path for live repos.

## Considered Options

- `bun build --compile` single binary: rejected — reintroduces the build step,
  and runtime `import()` of repo-local `.ts` plugins from a compiled binary is
  unproven (spike SIGKILLed on macOS, exit 137, unresolved); it couples a closed
  distribution to an open-world plugin model.
- Continue evolving the Rust codebase: rejected — the mentor critique (2/8/26)
  identified layering, not code quality, as the disease, but the iteration cost
  in Rust makes the re-layering slower than a rewrite in the target paradigm.

History note: maestro was originally TypeScript/bun, rewritten in Rust in
ebb80b01 (PR #67, 2026-06-03). This reverses that, deliberately.
