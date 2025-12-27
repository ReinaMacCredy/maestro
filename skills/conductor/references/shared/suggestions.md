# Suggestion Resolver

Reference for workflow state → next action suggestions.

## State to Suggestion Mapping

| Current State | Primary Suggestion | Alt Suggestion |
|---------------|-------------------|----------------|
| `INIT` | `ds (start design)` | — |
| `DESIGNED` | `/conductor-newtrack {id}` | `ds (refine design)` |
| `TRACKED` | `fb (file beads)` | `bd ready (if beads exist)` |
| `FILED` | `rb (review beads)` | — |
| `REVIEWED` | `bd ready (start work)` | — |
| `IMPLEMENTING` | `Start epic {next-epic-id}` or `{next-task-title}` | `finish branch (if all done)` |
| `DONE` | `finish branch` | — |
| `ARCHIVED` | `ds (start new work)` | — |

## Output Format

All commands should end with a suggestion block using this format:

```
┌─────────────────────────────────────────┐
│ ✓ {command} completed                   │
│                                         │
│ → Next: {primary_suggestion}            │
│   Alt: {alt_suggestion}                 │
└─────────────────────────────────────────┘
```

### Format Rules

1. **Box width**: Adapt to longest line, minimum 40 characters
2. **Primary suggestion**: Always show with `→ Next:` prefix
3. **Alt suggestion**: Only show if available, with `  Alt:` prefix (2-space indent)
4. **Success indicator**: `✓` for completed, `⚠` for warnings, `✗` for errors

## Resolver Logic

```
function getSuggestion(workflowState, trackId, beadsData):
    switch workflowState:
        case "INIT":
            return {
                primary: "ds (start design)",
                alt: null
            }
        
        case "DESIGNED":
            return {
                primary: "/conductor-newtrack " + trackId,
                alt: "ds (refine design)"
            }
        
        case "TRACKED":
            if beadsData?.epics?.length > 0:
                return {
                    primary: "bd ready",
                    alt: "fb (re-file beads)"
                }
            return {
                primary: "fb (file beads)",
                alt: null
            }
        
        case "FILED":
            return {
                primary: "rb (review beads)",
                alt: null
            }
        
        case "REVIEWED":
            return {
                primary: "bd ready (start work)",
                alt: null
            }
        
        case "IMPLEMENTING":
            nextTask = findNextReadyTask(beadsData)
            if nextTask:
                return {
                    primary: "Start epic " + nextTask.id,
                    alt: "finish branch (if all done)"
                }
            return {
                primary: "finish branch",
                alt: null
            }
        
        case "DONE":
            return {
                primary: "finish branch",
                alt: null
            }
        
        case "ARCHIVED":
            return {
                primary: "ds (start new work)",
                alt: null
            }
        
        default:
            return {
                primary: "/conductor-status",
                alt: null
            }
```

## Command-Specific Examples

### After `ds` (design session)

```
┌─────────────────────────────────────────────────────┐
│ ✓ Design session completed                          │
│                                                     │
│ Track: ux-automation_20251227                       │
│ Design: conductor/tracks/ux-automation_20251227/design.md │
│                                                     │
│ → Next: /conductor-newtrack ux-automation_20251227  │
│   Alt: ds (refine design)                           │
└─────────────────────────────────────────────────────┘
```

### After `/conductor-newtrack`

```
┌─────────────────────────────────────────┐
│ ✓ Track created                         │
│                                         │
│ Track: auth-system_20251228             │
│ Spec: conductor/tracks/.../spec.md      │
│ Plan: conductor/tracks/.../plan.md      │
│ Beads: 5 epics, 18 issues               │
│                                         │
│ → Next: rb (review beads)               │
└─────────────────────────────────────────┘
```

### After `fb` (file beads)

```
┌─────────────────────────────────────────┐
│ ✓ Beads filed                           │
│                                         │
│ Epics: 3                                │
│ Issues: 12                              │
│                                         │
│ → Next: rb (review beads)               │
└─────────────────────────────────────────┘
```

### After `rb` (review beads)

```
┌─────────────────────────────────────────┐
│ ✓ Beads reviewed                        │
│                                         │
│ Reviewed: 12                            │
│ Updated: 3                              │
│                                         │
│ → Next: bd ready (start work)           │
└─────────────────────────────────────────┘
```

### After `/conductor-implement` (epic complete)

```
┌─────────────────────────────────────────┐
│ ✓ Epic completed                        │
│                                         │
│ Epic: my-workflow:3-40yr.1              │
│ Tasks: 5/5 done                         │
│                                         │
│ → Next: Start epic my-workflow:3-40yr.2 │
│   Alt: rb (review remaining)            │
└─────────────────────────────────────────┘
```

### After `/conductor-finish`

```
┌─────────────────────────────────────────┐
│ ✓ Track completed                       │
│                                         │
│ Summary:                                │
│ - Threads processed: 3                  │
│ - Beads compacted: 12                   │
│ - Learnings added: 5                    │
│ - Location: conductor/archive/{id}/     │
│                                         │
│ → Next: ds (start new work)             │
└─────────────────────────────────────────┘
```

## Integration Points

Commands that need suggestion output:

| Command | Updates State To | Suggestion Source |
|---------|------------------|-------------------|
| `ds` | DESIGNED | This resolver |
| `/conductor-newtrack` | TRACKED or FILED | This resolver |
| `fb` | FILED | This resolver |
| `rb` | REVIEWED | This resolver |
| `/conductor-implement` | IMPLEMENTING or DONE | This resolver + bd ready |
| `/conductor-finish` | ARCHIVED | This resolver |

## Helper: Format Suggestion Box

```
function formatSuggestionBox(status, lines, suggestion):
    # Calculate width
    allLines = [status + " completed"] + lines + ["→ Next: " + suggestion.primary]
    if suggestion.alt:
        allLines.append("  Alt: " + suggestion.alt)
    
    width = max(len(line) for line in allLines) + 4  # 1 border + 1 space per side (total +4)
    width = max(width, 40)  # minimum width
    
    # Build box
    top = "┌" + "─" * width + "┐"
    bottom = "└" + "─" * width + "┘"
    
    result = [top]
    for line in allLines:
        result.append("│ " + line.ljust(width - 4) + " │")
    result.append(bottom)
    
    return "\n".join(result)
```
