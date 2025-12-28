# LEDGER.log Format

Append-only session log for tracking work progress.

## Format

```
# LEDGER.log - Append-only session log
# Format: ISO_TIMESTAMP | EVENT_TYPE | DATA
# Max 1000 entries, rotates to .log.1

2025-12-28T10:00:00Z | SESSION_START | track:skill-integration
2025-12-28T10:01:00Z | CLAIMED | issue:bd-42
2025-12-28T10:02:00Z | TDD_PHASE | RED | issue:bd-42
2025-12-28T10:05:00Z | TDD_PHASE | GREEN | issue:bd-42
2025-12-28T10:07:00Z | TDD_PHASE | REFACTOR | issue:bd-42
2025-12-28T10:10:00Z | PRE_VERIFY | PASS | issue:bd-42
2025-12-28T10:11:00Z | COMPLETED | issue:bd-42
2025-12-28T10:15:00Z | TRACK_COMPLETE | track:skill-integration
```

## Event Types

| Event | Data | Description |
|-------|------|-------------|
| SESSION_START | track:id | New session begins |
| CLAIMED | issue:id | Task claimed |
| TDD_PHASE | RED/GREEN/REFACTOR, issue:id | TDD state change |
| PRE_VERIFY | PASS/FAIL, issue:id | Pre-verification result |
| COMPLETED | issue:id | Task completed |
| RESERVED | files:[list] | Parallel mode file reservation |
| CONFLICT | files:[list] | Merge conflict detected |
| TRACK_COMPLETE | track:id | Track finished |

## Why Append-Only

- **Race-safe**: Multiple writers can append without conflicts
- **Recovery**: Easy to parse and filter
- **History**: Full audit trail of work
- **Simple**: No complex state management

## Writing

```bash
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) | EVENT | DATA" >> LEDGER.log
```

## Reading

```bash
# Find completed issues
grep "COMPLETED" LEDGER.log | cut -d'|' -f3 | tr -d ' '

# Find current session
tail -n1 LEDGER.log | grep SESSION_START

# TDD phase history for an issue
grep "bd-42" LEDGER.log | grep TDD_PHASE
```

## Location

```
conductor/sessions/active/LEDGER.log
```
