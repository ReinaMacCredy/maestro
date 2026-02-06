---
name: skill-matcher
description: Matches task descriptions against skill triggers and keywords. Returns ranked list of relevant skills.
user-invocable: false
---

# Skill Matcher

> Match tasks to skills using simple keyword-based matching.

## When to Use

This is an internal skill used by the orchestrator to find relevant skills for each task. It works with the skill registry output to identify which skills might help accomplish a given task.

## Input Format

The matcher expects:

1. **Task description** — A string describing what needs to be done
2. **Skill registry** — Array of skill objects from the registry:

```
[
  {
    "name": "skill-name",
    "description": "What the skill does",
    "triggers": ["trigger1", "trigger2"],  // optional
    "priority": 10                          // optional, default 0
  }
]
```

## Matching Algorithm

### Step 1: Normalize

- Convert task description to lowercase
- Split into words (alphanumeric tokens)

### Step 2: Match Each Skill

For each skill in the registry:

1. **Trigger match** (highest relevance):
   - If skill has `triggers` array, check if ANY trigger word appears in the task description
   - Match is case-insensitive
   - Partial word matches count (e.g., "testing" matches trigger "test")

2. **Keyword match** (lower relevance):
   - If no trigger match, check if skill `name` or words from `description` appear in task
   - Skip common words: "the", "a", "an", "to", "for", "and", "or", "is", "in", "on", "with"

### Step 3: Rank Results

Sort matched skills by:
1. **Priority** — Higher priority first (default: 0)
2. **Alphabetically** — By skill name when priority is equal

## Output Format

Returns an array of matched skills:

```
[
  {
    "name": "kraken",
    "relevance": "trigger",
    "priority": 10
  },
  {
    "name": "project-conventions",
    "relevance": "keyword",
    "priority": 0
  }
]
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Skill name from registry |
| `relevance` | `"trigger"` \| `"keyword"` | How the match was found |
| `priority` | number | Skill priority (for sorting context) |

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Empty task description | Return `[]` |
| Empty skill registry | Return `[]` |
| No matches found | Return `[]` |
| Skill has no triggers or description | Skip that skill |

## Example

**Input:**
- Task: "Write unit tests for the auth module"
- Registry: `[{name: "kraken", triggers: ["test", "tdd"], priority: 5}, {name: "spark", description: "Quick fixes"}]`

**Process:**
1. Task words: `["write", "unit", "tests", "for", "the", "auth", "module"]`
2. "kraken" — trigger "test" matches "tests" → `{name: "kraken", relevance: "trigger", priority: 5}`
3. "spark" — no trigger match, "quick" and "fixes" not in task → no match

**Output:**
```
[{name: "kraken", relevance: "trigger", priority: 5}]
```
