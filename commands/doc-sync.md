# Doc Sync

Sync AGENTS.md files by extracting knowledge from completed Amp threads.

## Usage

Run after completing an epic or anytime you want to capture learnings from past work:

- **After closing an epic**: Automatically extracts thread URLs from closed child issues
- **Manual sync**: Specify scope like `doc-sync for the auth epic`

## What It Does

1. **Finds closed issues** with thread URLs in their notes
2. **Reads threads** to extract patterns, commands, gotchas, and decisions
3. **Updates AGENTS.md** files nearest to the changed code
4. **Shows diff** for your review before committing

## Prerequisites

Thread URLs must be saved in beads during execution:
```bash
bd comment <id> "THREAD: https://ampcode.com/threads/T-xxx"
```

This uses atomic comment append (safer for multi-agent concurrency than `--notes`).

## Examples

```
/doc-sync                    # Sync recent closed issues
/doc-sync for the auth epic  # Sync specific epic
sync docs                    # Alternative trigger
```
