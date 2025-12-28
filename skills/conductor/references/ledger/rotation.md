# LEDGER.log Rotation

Auto-rotate when log exceeds 1000 entries.

## Rotation Logic

```bash
if [ $(wc -l < LEDGER.log) -gt 1000 ]; then
    # Rotate old archives
    [ -f LEDGER.log.1 ] && mv LEDGER.log.1 LEDGER.log.2
    
    # Archive current
    mv LEDGER.log LEDGER.log.1
    
    # Start fresh
    touch LEDGER.log
fi
```

## Pseudocode

```
if (lineCount(LEDGER.log) > 1000):
    mv LEDGER.log.1 LEDGER.log.2  # if exists
    mv LEDGER.log LEDGER.log.1
    touch LEDGER.log
```

## When to Check

- At session start (before first write)
- After each track completes

## File Structure

```
conductor/sessions/active/
├── LEDGER.log      # Current (0-1000 entries)
├── LEDGER.log.1    # Previous rotation
└── LEDGER.log.2    # Older rotation (optional)
```

## Why 1000 Entries

- Large enough for multi-session work
- Small enough to parse quickly
- Easy to grep through
- Keeps history accessible

## Archive Policy

- Keep `.log.1` and `.log.2`
- Delete older rotations if needed
- Or compress for long-term storage
