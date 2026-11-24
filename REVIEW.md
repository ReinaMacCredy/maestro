# Review

How a change to this repository is reviewed. The reviewer is usually an agent,
briefed with `maestro recipe show review`. It reads this file from the base
branch (`git show <base>:REVIEW.md`), never from the change under review, so
a change cannot relax its own review.

## Rounds

One reviewer, several rounds. Round one reads the whole diff. Each later
round receives the earlier findings and the new diff, and checks that each
finding is fixed or answered in writing before it looks for new ones. Every
finding carries one tag:

- `important`: a defect the change adds or makes worse. Blocks until fixed or
  answered in writing.
- `nit`: small, optional. Never blocks.
- `pre-existing`: was there before the change. Reported, never charged to the
  change.

A finding names the file and line, the input or state that goes wrong, and
what happens. A third failing round goes to the owner.

## Reliability checklist

One line per item: the trigger as paths or patterns readable from the diff,
the check as an action or command, and the incident behind it. Apply only the
items whose trigger matches the diff. A processed lesson may end as a new line
here; a line that has not matched a diff in a year is a candidate to delete.

| Trigger | Check | Incident |
|---|---|---|
| A new file under `src/plugins/recipes/` or `src/plugins/skills/`, or a new `register(` / `registerSessionCommand(` call | The change names what the new verb, recipe or skill replaces, or cites the evidence that it is needed. Refuse otherwise. | Markdown under `src` grew from 1,246 lines (v0.108.0) to 9,096 (v0.126.0); `bun scripts/usage-count.ts` then found most of it uncalled (d912). |
| `tests/differential/approved-changes.txt` | Each added line is its own commit, cites a locked decision, and rests on the owner's word quoted in the commit or the work note. Run `bun scripts/differential.ts --base <base>`. | The list is the only thing between a behaviour change and the oracle (d914). |
| `context.store.migrate(` added or changed, a `CREATE TABLE` removed, any `DROP` or `DELETE FROM` in `src/` | Run `bun scripts/differential.ts --stores <a copy of a real store>`; no table and no row may be lost. | Stored data stays readable across versions (d876). |
| Code inside a plugin's `apply(` that queries or loops over rows | Time `maestro version` in a store with thousands of rows before and after. `apply` runs on every invocation. | Terms, bundle snapshots and memory facts were re-indexed on every command: 232 of 293 ms. |
| `process.kill(`, `pid`, `isAlive`, `liveness` in `src/` | Ask what happens when the pid now belongs to another process. Run `bun test tests/session-liveness.test.ts`. | Sessions last seen 31 to 57 days earlier showed as live and kept their leases through reused pids. |
| Shipped markdown under `src/plugins/` that names a `maestro <verb>` command | Run `bun test tests/docs-contract.test.ts` and try the named command once. | Agents ran `maestro work update` and `maestro decision add` because text or habit suggested verbs that did not exist. |
| A test file deleted, a test removed, or an expectation changed | Each one is named in the handback with the feature it left with or the contract that changed. A test never leaves to make a run pass or faster. | d877. |
| `src/kernel/` | Run the A2 grep from `.github/workflows/ci.yml`; the kernel holds mechanism only. | CI gate A2. |
| `.claude/` or `.codex/` | Refuse a hand edit; change `src/plugins/install.ts` and reinstall. | The installer owns the wiring and overwrites it. |
| `setTimeout`, `setInterval`, `spawn(` with `detached` in `src/` | Run the A1 grep; no daemon, scheduler or detached process. | CI gate A1. |
