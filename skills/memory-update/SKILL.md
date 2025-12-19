# Memory Update

Use when: User wants to persist information across sessions via memory blocks, compact scratch memory, or search memory.

## Trigger Phrases

- `remember [something]` - Add to memory
- `/compact` - Archive scratch, migrate facts
- `/memory-search [query]` - Semantic search via LanceDB

## Memory Block Structure

Memory blocks are stored in `.memory/` directory:

```
.memory/
├── core.md      # Fundamental project facts (rarely changes)
├── user.md      # User preferences, style, conventions
├── project.md   # Project-specific context, architecture
└── scratch.md   # Current session notes (volatile)
```

## Commands

### `remember [fact]`

Add information to appropriate memory block.

**Classification:**
- Core facts → `core.md` (project name, primary language, critical constraints)
- User preferences → `user.md` (coding style, tool preferences, workflow choices)
- Project context → `project.md` (architecture decisions, module purposes, integration patterns)
- Temporary notes → `scratch.md` (current task context, WIP notes)

**Format:**
```markdown
## [Category]

- [YYYY-MM-DD] <fact>
```

**Example:**
```
User: remember that we use pnpm not npm
→ Adds to user.md:
## Package Manager
- [2024-01-15] Project uses pnpm, not npm
```

### `/compact`

Archive scratch memory and migrate durable facts.

**Workflow:**
1. Read `scratch.md`
2. Identify durable facts (will be relevant next session)
3. Migrate durable facts to appropriate block (core/user/project)
4. Archive remaining scratch to `history/memory-archive/scratch-{date}.md`
5. Clear `scratch.md`

**Archive format:**
```markdown
# Scratch Archive - {date}
Session thread: {thread-id}

[archived content]
```

### `/memory-search [query]`

Semantic search across all memory blocks.

**Implementation:**
```bash
# Using lib/memory_search.py with LanceDB
python lib/memory_search.py --query "<query>" --index .memory-index/memory.lance
```

**Returns:**
- Top 5 relevant memory entries
- Source file and line
- Relevance score

## Memory Capacity Management

**Threshold warning at 80%:**
- core.md: 50 lines max
- user.md: 100 lines max  
- project.md: 200 lines max
- scratch.md: 500 lines max (compacted regularly)

When approaching threshold:
1. Warn user
2. Suggest compaction
3. Identify redundant/outdated entries

## Session Integration

**Session start (via hook):**
```
1. Load all memory blocks as XML context
2. Check capacity thresholds
3. Warn if scratch.md is large
```

**Session end:**
```
1. Prompt for `/compact` if scratch.md has content
2. Update project.md with session learnings
```

## Index Management

**Rebuild index after updates:**
```bash
python lib/memory_search.py --rebuild --source .memory/ --index .memory-index/memory.lance
```

## Output Format

### remember
```
ADDED to [block]: <summary>
```

### /compact
```
ARCHIVED: scratch.md → history/memory-archive/scratch-{date}.md
MIGRATED: <count> facts to core/user/project
CLEARED: scratch.md
```

### /memory-search
```
RESULTS for "<query>":
1. [project.md:L42] <relevant text> (score: 0.89)
2. [user.md:L15] <relevant text> (score: 0.76)
...
```
