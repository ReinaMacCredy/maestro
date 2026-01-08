# Oracle Reconciliation

The Oracle compares three truth sources to identify documentation gaps, staleness, and required updates.

## Three-Source Comparison

| Source | Contains | Priority |
|--------|----------|----------|
| **CODE** | Current implementation state | Truth (highest) |
| **TOPICS** | Extracted knowledge from threads | Intent |
| **DOCS** | Existing documentation content | Baseline |

```
TOPICS ←→ CODE ←→ DOCS
   ↑         ↑       ↑
  intent   truth   current
```

## Oracle Reconciliation Prompt

```
Compare:
1. TOPICS: [extracted knowledge]
2. CODE: [verified state with file paths]
3. DOCS: [current documentation content]

Identify:
- GAPS: Knowledge in topics not in docs
- STALE: Docs that contradict current code
- CONFLICTS: Topics vs docs disagreements

Output JSON:
{
  "gaps": [{"topic": "...", "target_file": "...", "section": "..."}],
  "stale": [{"file": "...", "issue": "...", "correction": "..."}],
  "conflicts": [{"topic_says": "...", "doc_says": "...", "resolution": "..."}],
  "updates": [
    {
      "file": "path/to/doc.md",
      "action": "add|update|remove",
      "section": "section name",
      "content": "what to write",
      "rationale": "why"
    }
  ]
}
```

## Output Categories

### GAPS
Knowledge extracted from threads that doesn't exist in documentation:

```json
{
  "gaps": [
    {
      "topic": "JWT migration",
      "target_file": "AGENTS.md",
      "section": "Authentication"
    }
  ]
}
```

### STALE
Documentation that contradicts current code state:

```json
{
  "stale": [
    {
      "file": "docs/auth.md",
      "issue": "Describes cookie-based auth",
      "correction": "Now uses JWT tokens"
    }
  ]
}
```

### UPDATES
Specific changes to apply:

```json
{
  "updates": [
    {
      "file": "AGENTS.md",
      "action": "add",
      "section": "Authentication",
      "content": "JWT tokens via `JWTService` class",
      "rationale": "Document new auth flow"
    }
  ]
}
```

## Apply Strategies

### Text Updates

Use `edit_file` with surgical precision:

```
1. Read target file first
2. Note structure, sections, voice
3. Identify precise insertion/edit point
4. Apply minimal change preserving style
```

**Anti-patterns:**
- ❌ Wholesale file replacement
- ❌ Changing existing terminology
- ❌ Breaking section hierarchy

### Mermaid Diagrams

Generate diagrams with code citations:

```json
{
  "code": "flowchart LR\n  A[Client] --> B[RetryPolicy]\n  B --> C[APIHandler]",
  "citations": {
    "Client": "file:///src/api/client.ts#L10",
    "RetryPolicy": "file:///src/api/retry.ts#L45",
    "APIHandler": "file:///src/api/handler.ts#L1"
  }
}
```

Citations enable:
- Click-through to source
- Verification of accuracy
- Stale detection on code changes

### Parallel Updates

When updating multiple unrelated files:

```
Task A: Update AGENTS.md auth section
Task B: Update docs/architecture.md
Task C: Generate mermaid flow diagram
```

## Conflict Resolution

| Conflict Type | Resolution |
|---------------|------------|
| Code ≠ Topic | Code wins (topic may be outdated) |
| Code ≠ Docs | Update docs to match code |
| Topic ≠ Docs | Verify against code, then decide |

## Quality Checklist

```
- [ ] All gaps have target file + section
- [ ] Stale items have specific corrections
- [ ] Updates preserve existing doc voice
- [ ] Mermaid diagrams have citations
- [ ] Changes are surgical, not wholesale
```
