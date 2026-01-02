# Orchestrator Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ORCHESTRATOR                                   │
│                              (Main Agent)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. Read plan.md Track Assignments                                          │
│  2. Initialize Agent Mail                                                   │
│  3. Spawn workers via Task()                                                │
│  4. Monitor progress via fetch_inbox                                        │
│  5. Handle cross-track blockers                                             │
│  6. Announce completion                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           │ Task() spawns parallel workers
           ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  Worker A        │  │  Worker B        │  │  Worker C        │
│  Track 1         │  │  Track 2         │  │  Track 3         │
├──────────────────┤  ├──────────────────┤  ├──────────────────┤
│  For each bead:  │  │  For each bead:  │  │  For each bead:  │
│  • Reserve files │  │  • Reserve files │  │  • Reserve files │
│  • bd claim      │  │  • bd claim      │  │  • bd claim      │
│  • Do work       │  │  • Do work       │  │  • Do work       │
│  • bd close      │  │  • bd close      │  │  • bd close      │
│  • Report mail   │  │  • Report mail   │  │  • Report mail   │
└──────────────────┘  └──────────────────┘  └──────────────────┘
           │                   │                   │
           └───────────────────┼───────────────────┘
                               ▼
                    ┌─────────────────────┐
                    │     Agent Mail      │
                    │  ─────────────────  │
                    │  Epic Thread:       │
                    │  • Progress reports │
                    │  • Bead completions │
                    │  • Blockers         │
                    └─────────────────────┘
```

## Key Difference from /conductor-implement

| Aspect | /conductor-implement | /conductor-orchestrate |
|--------|---------------------|----------------------|
| Execution | Sequential, main agent | Parallel, worker subagents |
| bd access | Main agent only | **Workers CAN claim/close** |
| Coordination | N/A | Agent Mail MCP |
| File locking | N/A | file_reservation_paths |
| Context | In-memory | Track threads (persistent) |

## Auto-Orchestration Integration

When triggered from `fb` (file beads) auto-orchestration:

1. Track Assignments are **auto-generated** from beads dependency graph
2. No manual Track Assignments section needed in plan.md
3. Orchestrator receives assignments via in-memory call, not file parsing

### Auto-Generated vs Manual

| Source | How Detected | Behavior |
|--------|--------------|----------|
| Auto-generated | Called from fb Phase 6 | Assignments passed in-memory |
| Manual | User runs `/conductor-orchestrate` | Parse from plan.md |

Both flows converge at Phase 3 (Spawn Workers).

## Directory Structure

```
skills/orchestrator/
├── SKILL.md           # Main skill file
├── agents/            # Agent profiles by category
│   ├── research/      # Locator, Analyzer, Pattern, Web, GitHub
│   ├── review/        # CodeReview, SecurityAudit, PerformanceReview
│   ├── planning/      # Architect, Planner
│   ├── execution/     # Implementer, Modifier, Fixer, Refactorer
│   └── debug/         # Debugger, Tracer
├── references/        # Workflow documentation
│   ├── workflow.md    # 8-phase protocol
│   ├── preflight.md   # Session Brain preflight
│   ├── worker-prompt.md
│   └── patterns/
└── scripts/           # Session brain utilities
    └── preflight.py   # Preflight protocol implementation
```

## Lazy Loading Requirements

Host-side changes required for Maestro loader to support deferred reference loading.

### Overview

The loader currently loads all skill content eagerly. To reduce token consumption and improve startup time, the loader should support **lazy loading** of references based on workflow phase or trigger conditions.

### Host-Side Loader Changes

#### 1. Load SKILL.md Only (Core Instructions)

```python
# Current behavior: loads SKILL.md + all references
skill_content = load_skill("orchestrator")  # Loads everything

# New behavior: loads SKILL.md only
skill_content = load_skill("orchestrator")  # Loads SKILL.md core
# References loaded separately on-demand
```

#### 2. Parse `## Lazy References` Table

The loader must detect and parse the `## Lazy References` section in SKILL.md:

```markdown
## Lazy References

| When | Load |
|------|------|
| Phase 4 (spawn) | [worker-prompt.md](references/worker-prompt.md) |
| Cross-track deps | [agent-coordination.md](references/agent-coordination.md) |
| Conflict resolution | [agent-mail.md](references/agent-mail.md) |
```

**Parsing Requirements:**
- Detect `## Lazy References` heading (exact match, case-sensitive)
- Parse markdown table with `When | Load` columns
- Extract trigger condition from `When` column
- Extract file path from markdown link in `Load` column
- Store as trigger → file path mapping

#### 3. Load References on Trigger Activation

When a specific phase or trigger activates, inject the corresponding reference:

```python
# Trigger conditions (examples)
TRIGGER_PATTERNS = {
    "Phase 4": ["spawn", "Task(", "worker"],
    "Cross-track deps": ["dependency", "blocked", "waiting"],
    "Conflict resolution": ["conflict", "file_reservation", "overlap"],
}

def check_trigger(context: str, trigger: str) -> bool:
    """Check if trigger condition is met based on context."""
    patterns = TRIGGER_PATTERNS.get(trigger, [])
    return any(p in context for p in patterns)

def lazy_load_if_needed(skill: str, context: str):
    """Load deferred references when trigger activates."""
    for trigger, ref_path in skill.lazy_references.items():
        if check_trigger(context, trigger):
            inject_reference(ref_path)
```

#### 4. Backwards Compatibility

Support both eager and deferred loading patterns:

| Section | Behavior | Use Case |
|---------|----------|----------|
| `## References` | **Eager** - Load with SKILL.md | Critical references always needed |
| `## Lazy References` | **Deferred** - Load on trigger | Phase-specific or rare-path references |

**Migration Path:**
1. Existing skills with only `## References` continue to work unchanged
2. Skills can add `## Lazy References` section incrementally
3. No breaking changes to existing skill format

### Trigger Detection Strategies

| Strategy | Description | Complexity |
|----------|-------------|------------|
| **Keyword match** | Match trigger patterns in user input/context | Low |
| **Phase tracking** | Track workflow phase via state machine | Medium |
| **Semantic match** | Use embeddings to match intent to trigger | High |

**Recommended:** Start with keyword match, add phase tracking for orchestrator.

### Example Loader Implementation

```python
class SkillLoader:
    def load(self, skill_name: str) -> LoadedSkill:
        """Load skill with lazy reference support."""
        skill_path = self.resolve_skill(skill_name)
        
        # Step 1: Load SKILL.md only
        content = read_file(skill_path / "SKILL.md")
        
        # Step 2: Parse sections
        eager_refs = self.parse_references(content)      # ## References
        lazy_refs = self.parse_lazy_references(content)  # ## Lazy References
        
        # Step 3: Load eager references immediately
        for ref in eager_refs:
            content += read_file(skill_path / ref)
        
        return LoadedSkill(
            content=content,
            lazy_refs=lazy_refs,  # Deferred until trigger
        )
    
    def parse_lazy_references(self, content: str) -> dict[str, str]:
        """Parse ## Lazy References table into trigger → path mapping."""
        # Find section
        match = re.search(r"## Lazy References\n\n\|.*\n\|.*\n((?:\|.*\n)*)", content)
        if not match:
            return {}
        
        # Parse table rows
        refs = {}
        for line in match.group(1).strip().split("\n"):
            parts = line.split("|")
            if len(parts) >= 3:
                trigger = parts[1].strip()
                load_cell = parts[2].strip()
                # Extract path from markdown link [text](path)
                path_match = re.search(r"\[.*\]\((.*)\)", load_cell)
                if path_match:
                    refs[trigger] = path_match.group(1)
        return refs
```

### Acceptance Criteria

- [ ] Loader parses `## Lazy References` table from SKILL.md
- [ ] Eager references (`## References`) load immediately with skill
- [ ] Lazy references load only when trigger condition activates  
- [ ] Existing skills without `## Lazy References` work unchanged
- [ ] Token savings measurable (target: 40-60% reduction for complex skills)
