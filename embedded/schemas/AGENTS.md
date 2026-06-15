# Embedded Schemas Agent Notes

## OVERVIEW

`embedded/schemas/` is the shipped schema-pack source for Maestro artifact
families. Rust is the interpreter; YAML/JSON fixtures are reviewable contracts.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Current artifact shape | `<family>/current.yaml` | One or more stamped contracts for persisted files or entries. |
| Read/write compatibility | `<family>/supported.yaml` | Bounded read set, write version, and explicit legacy migrate routes. |
| Reserved names | `<family>/retired.yaml` | Version stamps and fields that must not be reused. |
| Fixture coverage | `<family>/fixtures/` | Read through real entry points; not standalone examples. |
| Rust parser/validator | `src/domain/schema_contracts/` | Keeps packs consistent with current constants and consumers. |

## CONVENTIONS

- Every family is a pack: `current.yaml`, `supported.yaml`, `retired.yaml`,
  and `fixtures/`.
- Schema packs describe contracts; domain readers and migrations still define
  trusted behavior.
- Fixture reads must not rewrite seeded bytes.
- If a schema pack changes, update the corresponding tree hash/version guard in
  `tests/resources_version_guard.rs`.

## ANTI-PATTERNS (THIS PROJECT)

- Do not add a field to YAML without updating the owning Rust model and tests.
- Do not remove or reuse retired names.
- Do not treat fixtures as migration output unless the real reader/writer path
  proves the same behavior.

## VERIFICATION

Run `tests/schema_contracts_validation.rs`,
`tests/schema_fixture_harness.rs`, and `tests/resources_version_guard.rs`.
Broaden to the owning domain or command integration test for the changed
family.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
