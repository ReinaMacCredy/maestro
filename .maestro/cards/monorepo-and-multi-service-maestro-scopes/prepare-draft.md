# Prepare plan for monorepo-and-multi-service-maestro-scopes

# Review before applying. Split this into multiple tasks only when the
# accepted contract has independent work slices; add blocker: lines only
# for real approvals or external waits.
# Affected areas: src/domain/card/schema.rs (project on Card base), src/domain/card/store.rs (auto-infer on create + --project override), src/domain/harness/schema.rs (projects: layout declaration, forward-compat), src/foundation/core/paths.rs (folder->project resolution vs repo root), src/interfaces/cli (list/ready --project filter, project badge, group-by-project), JSON encoding (dense flat project field on list/ready/status), src/operations/init + sync (preserve projects: config across lifecycle), embedded/skills/maestro-setup/SKILL.md (read-in doc/agent-spec enumeration per project; version bump + guard re-record)

## Task T1: Implement accepted behavior
covers: ac-1, ac-2, ac-3, ac-4, ac-5, ac-6, ac-7, ac-8, ac-9, ac-10, ac-11, ac-12, ac-13, ac-14, ac-15
check: Card-base project field: a card created with --project svc-pay persists 'project: svc-pay' in .maestro/cards/<id>/card.yaml for EVERY card type (feature/task/bug/chore/idea/decision); a card created without it has no project key; pre-existing cards lacking the field still load (additive, round-trip safe via the existing extra/unknown forward-tolerance).
