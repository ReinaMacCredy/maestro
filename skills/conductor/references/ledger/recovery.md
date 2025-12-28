# Session Recovery

Resume interrupted sessions using LEDGER.log.

## Recovery Logic

```bash
# On session resume:
completed=$(grep "COMPLETED" LEDGER.log | awk -F'|' '{print $3}' | tr -d ' ')

for issue in $(bd ready --json | jq -r '.[].id'); do
    if echo "$completed" | grep -q "$issue"; then
        echo "Skip: $issue (already completed)"
    else
        echo "Work: $issue"
        bd update $issue --status in_progress
        # ... do work
    fi
done
```

## Pseudocode

```
# On session resume:
completed = grep "COMPLETED" LEDGER.log | extract issue:id
for issue in epic.issues:
    if issue.id in completed:
        skip
    else:
        claim and work
```

## Recovery Scenarios

### Crash Mid-Task

Log shows:
```
2025-12-28T10:01:00Z | CLAIMED | issue:bd-42
2025-12-28T10:02:00Z | TDD_PHASE | RED | issue:bd-42
```

No COMPLETED entry → resume from RED phase.

### Crash After Complete

Log shows:
```
2025-12-28T10:01:00Z | CLAIMED | issue:bd-42
2025-12-28T10:11:00Z | COMPLETED | issue:bd-42
```

COMPLETED entry → skip this issue.

### Session Restart

Log shows:
```
2025-12-28T10:00:00Z | SESSION_START | track:skill-integration
2025-12-28T10:11:00Z | COMPLETED | issue:bd-42
2025-12-28T10:25:00Z | COMPLETED | issue:bd-43
```

Resume with bd-44 (first uncompleted).

## Finding Resume Point

```bash
# Last claimed but not completed
last_claimed=$(grep "CLAIMED" LEDGER.log | tail -1 | awk -F'|' '{print $3}' | tr -d ' ')
is_completed=$(grep "COMPLETED.*$last_claimed" LEDGER.log)

if [ -z "$is_completed" ]; then
    echo "Resume: $last_claimed"
else
    echo "Start next issue"
fi
```

## TDD Phase Recovery

If crashed during TDD cycle:

```bash
# Find last TDD phase
last_phase=$(grep "TDD_PHASE.*bd-42" LEDGER.log | tail -1 | awk -F'|' '{print $3}' | tr -d ' ')

case $last_phase in
    RED) echo "Resume: write implementation" ;;
    GREEN) echo "Resume: refactor" ;;
    REFACTOR) echo "Resume: verify and close" ;;
esac
```
