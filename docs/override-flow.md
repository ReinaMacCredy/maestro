# Verdict Override Flow

A verdict override is an append-only audit record. It does not rewrite the original verdict or change the PR check conclusion. It is a waiver that says "a human with appropriate authorization reviewed this and accepted the risk."

---

## When to Override

Use `maestro verdict override` when:

- The PR is blocked by a `sensitive-paths` finding and the change is intentional and reviewed.
- A CI gate failed transiently and the root cause is confirmed non-code-related.
- An emergency hotfix requires merging before the full verification loop can complete.

Do not use overrides to silence findings that indicate real problems. Override authorization is logged, audited, and visible in the PR check summary.

---

## Who Can Override

Authorization is governed by `owners.yaml` under the `sensitive_waiver` role.

```yaml
# .maestro/policies/owners.yaml
sensitive_waiver:
  - alice
  - bob
```

The invoking user (resolved from `os.userInfo().username`) must appear in this list. If they are not listed, the command exits 1 with `not-authorized`.

**Rule 12: owners.yaml is always loaded from the base branch, not the PR head.** This prevents self-promotion: a contributor cannot add themselves to `sensitive_waiver` on their PR branch and then authorize their own override. The `--base <ref>` flag controls which ref is used to read `owners.yaml` (default: `main`).

```bash
# Default: loads owners.yaml from main
maestro verdict override --task <id> --pr <n> --reason "<why>"

# Explicit base
maestro verdict override --task <id> --pr <n> \
  --base origin/release/v2 --reason "<why>"
```

---

## Audit Trail

Every override call writes an Evidence row:

| Field | Value |
|---|---|
| `kind` | `verdict-override` |
| `witness_level` | `agent-claimed-and-not-reproducible` |
| `payload.verdictId` | The verdict being overridden |
| `payload.overriddenBy` | The invoking username |
| `payload.reason` | Free-text reason (required, no limit) |

The Evidence row is stored at `.maestro/evidence/` (gitignored) and is included in `maestro evidence list --task <id>` output.

---

## No-Silent-Pass Guarantees

An override does not make a blocked PR green in CI. Specifically:

1. **PR check conclusion is unchanged.** The GitHub Check posted by `maestro ci verify` still reflects the original verdict decision. A `BLOCK` verdict that was overridden still shows `action_required` in the PR check UI. The check summary line notes the override.
2. **Original verdict is immutable.** Verdicts are append-only. The override record is a separate Evidence row; the verdict object on disk is not mutated.
3. **Override is visible.** `maestro verdict show --task <id>` lists any overrides associated with the latest verdict. The PR check summary rendered by `maestro ci verify` includes a line identifying any overrides by username and reason.

An override is a human accountability record, not a CI bypass mechanism. If you need to actually unblock a merge, the appropriate action is to fix the root cause or to use your Git platform's admin bypass for the PR check (which is a separate platform-level action outside Maestro).

---

## Example

```bash
# Emergency hotfix: sensitive path change reviewed by on-call lead
maestro verdict override \
  --task tsk-aaaaaa \
  --pr 42 \
  --reason "Emergency hotfix for login breakage, reviewed and approved by on-call lead @alice"

# Override a specific older verdict (not latest)
maestro verdict override \
  --task tsk-aaaaaa \
  --pr 42 \
  --verdict vrd-bbbbbb \
  --reason "Manual sign-off after post-incident review"
```

Output confirms the Evidence row ID, who overrode, and repeats the no-silent-pass note:

```
[ok] Verdict override recorded: evd-xxxxxxxx
  Task:      tsk-aaaaaa
  Verdict:   vrd-bbbbbb
  By:        alice
  Reason:    Emergency hotfix ...
  Witness:   agent-claimed-and-not-reproducible
  Created:   2026-05-05T10:00:00Z

Note: the original verdict conclusion is unchanged. This override
is an audit record only. CI gate status is not affected.
```

---

## L7.9 Follow-Up: GitHub-Author Identity Validation

The current implementation resolves the invoking user from `os.userInfo().username` — the local OS user. This is suitable for local operator use but does not validate that the OS username corresponds to a GitHub identity.

L7.9 is planned to cross-validate the local username against the GitHub API (via the authenticated token identity on the PR's base repo) so that `sensitive_waiver` list entries map to verified GitHub handles rather than arbitrary local usernames. Until L7.9 ships, teams should treat `sensitive_waiver` entries as local usernames and ensure those usernames are consistent across all machines where overrides are invoked.

---

## Reference

- Who can override — `docs/owners-yaml-format.md`
- Auto-merge eligibility waiver — `docs/auto-merge-eligibility.md` (predicate 5: `sensitive-paths-untouched-without-waiver`)
- Source — `src/features/verdict/commands/verdict.command.ts` (`override` sub-command)
