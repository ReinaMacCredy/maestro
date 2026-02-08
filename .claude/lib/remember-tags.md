# Remember Tags Protocol

Workers can persist learnings, decisions, and issues discovered during execution using `<remember>` tags in their output.

## Tag Format

```
<remember category="learning|decision|issue">content</remember>
```

### Categories

| Category | Use When | Example |
|----------|----------|---------|
| `learning` | You discover something useful about the codebase, tool, or pattern | `<remember category="learning">The project uses barrel exports in index.ts files</remember>` |
| `decision` | You make a non-obvious implementation choice | `<remember category="decision">Used Map instead of Object for O(1) lookup on large datasets</remember>` |
| `issue` | You find a problem that isn't in your current task scope | `<remember category="issue">The auth middleware doesn't handle expired refresh tokens</remember>` |

## How It Works

1. Worker emits `<remember>` tags in their regular output
2. The `remember-extractor.sh` PostToolUse hook scans agent output for tags
3. Extracted content is appended to `.maestro/wisdom/{active-plan}.md` with timestamps
4. The orchestrator and future sessions can reference accumulated wisdom

## Guidelines

- Keep content concise (1-2 sentences)
- One insight per tag
- Use the most specific category that applies
- Do not use remember tags for routine status updates
