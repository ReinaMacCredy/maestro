# src Agent Notes

## OVERVIEW

`src/` is the Rust crate. Keep adapter parsing/rendering separate from domain
contracts, operation workflows, and foundation safety primitives.

## STRUCTURE

```text
src/
├── domain/       # durable local-first concepts and artifact contracts
├── operations/   # cross-domain workflows and orchestration
├── interfaces/   # CLI, MCP, hook, shell, and TUI adapters
├── foundation/   # shared core primitives
├── lib.rs        # public crate roots and compatibility re-exports
└── main.rs       # binary entrypoint and passive auto-check gate
```

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Add a concept-owned rule | `domain/` | Prefer a facade over adapter-side duplication. |
| Coordinate multiple concepts | `operations/` | Keep transaction and safety behavior explicit. |
| Parse/render an external surface | `interfaces/` | Parse args/messages, call facades, render output. |
| Add shared path/write/schema logic | `foundation/core/` | Verify every dependent safety surface. |

## CONVENTIONS

- New production imports should prefer target roots over compatibility shims.
- Parent `mod.rs` files are caller-facing facades; deep leaf modules stay
  private unless the architecture spec exposes them.
- `src/main.rs` should stay thin: parse, dispatch, map errors, and handle
  passive update checks.

## ANTI-PATTERNS (THIS PROJECT)

- Do not make adapters own durable artifact layout.
- Do not move behavior across Task, Proof, Run, Install, Migration, or Update
  without preserving concurrency, rollback, and schema checks.
- Do not use broad text search as proof of callers or dependencies; use
  `srcwalk trace` or `srcwalk deps`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [domain/AGENTS.md](domain/AGENTS.md)
- [foundation/core/AGENTS.md](foundation/core/AGENTS.md)
- [interfaces/cli/AGENTS.md](interfaces/cli/AGENTS.md)
- [operations/AGENTS.md](operations/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
