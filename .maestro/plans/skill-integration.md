# Skill Integration Plan

## Objective

Enable Maestro to discover locally installed Claude Code plugins and automatically delegate tasks to them based on keyword/capability matching. Maestro becomes a composition layer that orchestrates skills from any source.

## Scope

**In**:
- Local plugin discovery (scan standard locations)
- Skill registry (file-based, refreshable)
- Auto-detection in Prometheus (keyword + capability matching)
- Full delegation mechanism (Task-based invocation)

**Out**:
- Marketplace integration (future)
- Partial delegation modes
- Remote service authentication
- Skill versioning

---

## Phase 1: Local Plugin Discovery

### Task 1.1: Define plugin scan locations

Create a list of standard Claude Code plugin locations to scan.

**Locations to scan:**
```
~/.claude/plugins/           # User-installed plugins
~/.claude/skills/            # User skills (legacy location)
./.claude/skills/            # Project-local skills
./.claude-plugin/            # Current project if it's a plugin
```

**Files:**
- Create `scripts/discover-plugins.sh` - Shell script to find plugin manifests

**Acceptance criteria:**
- Script outputs JSON array of discovered plugin paths
- Handles missing directories gracefully
- Runs in under 2 seconds

### Task 1.2: Parse plugin manifests

Extract capability information from discovered plugins.

**Parse from `plugin.json`:**
```json
{
  "name": "frontend-design",
  "description": "...",
  "keywords": ["frontend", "ui", "design", "react", "tailwind"],
  "capabilities": ["generate-component", "design-page", "style-element"],
  "agents": "./.claude/agents/",
  "commands": "./.claude/commands/"
}
```

**Parse from `SKILL.md` (fallback):**
- Extract name from H1 heading
- Extract keywords from "Keywords:" or "Tags:" line
- Extract capabilities from "Capabilities:" section

**Files:**
- Create `scripts/parse-plugin.sh` - Parse single plugin manifest

**Acceptance criteria:**
- Returns normalized JSON with: name, description, keywords, capabilities, path
- Handles both plugin.json and SKILL.md formats
- Graceful degradation for incomplete manifests

### Task 1.3: Create discovery command

Add a `/discover` command to manually trigger plugin discovery.

**Files:**
- Create `.claude/commands/discover.md`

**Behavior:**
```
/discover           # Scan and display found plugins
/discover --refresh # Force refresh of registry
```

**Output format:**
```
Found 3 plugins:

  maestro (local)
    Keywords: workflow, planning, tdd
    Capabilities: interview, orchestrate, implement

  frontend-design (~/.claude/plugins/frontend-design)
    Keywords: frontend, ui, react, tailwind
    Capabilities: generate-component, design-page

  api-builder (~/.claude/plugins/api-builder)
    Keywords: api, rest, graphql, backend
    Capabilities: generate-endpoint, scaffold-api
```

**Acceptance criteria:**
- Runs discovery scripts
- Displays human-readable output
- Updates registry file

---

## Phase 2: Skill Registry

### Task 2.1: Define registry schema

Create the registry file structure.

**Location:** `.maestro/registry/skills.json`

**Schema:**
```json
{
  "version": "1.0",
  "updated_at": "2024-02-06T12:00:00Z",
  "skills": [
    {
      "name": "frontend-design",
      "source": "local",
      "path": "/Users/user/.claude/plugins/frontend-design",
      "keywords": ["frontend", "ui", "design", "react"],
      "capabilities": ["generate-component", "design-page"],
      "agents": ["designer", "stylist"],
      "commands": ["/design-component", "/style"],
      "priority": 100
    }
  ]
}
```

**Files:**
- Create `.maestro/registry/.gitkeep`
- Document schema in `.maestro/registry/README.md`

**Acceptance criteria:**
- Schema supports all required fields
- Includes source tracking (local, marketplace, manual)
- Priority field for conflict resolution

### Task 2.2: Implement registry refresh

Create mechanism to rebuild registry from discovered plugins.

**Files:**
- Create `scripts/refresh-registry.sh`

**Behavior:**
1. Run discovery scripts
2. Parse each discovered plugin
3. Merge into registry (preserve manual entries)
4. Write to `.maestro/registry/skills.json`

**Acceptance criteria:**
- Idempotent (same input = same output)
- Preserves manually-added skills
- Logs changes (added, removed, updated)

### Task 2.3: Registry validation

Add validation for registry integrity.

**Files:**
- Create `scripts/validate-registry.sh`

**Checks:**
- All paths exist and are accessible
- No duplicate skill names
- Required fields present
- Keywords are lowercase, no spaces

**Acceptance criteria:**
- Exit 0 if valid, exit 1 if invalid
- Human-readable error messages
- Can be run as pre-commit hook

---

## Phase 3: Auto-Detection in Prometheus

### Task 3.1: Load registry in Prometheus

Modify Prometheus agent to read skill registry at startup.

**Files:**
- Modify `.claude/agents/prometheus.md`

**Add to Prometheus workflow:**
```markdown
## Skill Registry

Before interviewing, check `.maestro/registry/skills.json` for available skills.

If a skill matches the request keywords:
1. Inform user: "I found a skill that can help: {skill_name}"
2. Ask if they want to delegate or proceed with Maestro
3. If delegate: use full delegation mode
```

**Acceptance criteria:**
- Registry loaded before interview starts
- Missing registry handled gracefully (empty = no external skills)
- Skill matches logged for debugging

### Task 3.2: Implement keyword matching

Add keyword extraction and matching logic.

**Matching algorithm:**
1. Tokenize user request (split on whitespace, lowercase)
2. For each registered skill:
   - Count keyword matches
   - Score = matches / total_keywords
3. Return skills with score > 0.3, sorted by score

**Example:**
```
Request: "design a frontend page for user login"
Tokens: ["design", "frontend", "page", "user", "login"]

frontend-design:
  keywords: ["frontend", "ui", "design", "react"]
  matches: ["design", "frontend"] = 2/4 = 0.5 MATCH

api-builder:
  keywords: ["api", "rest", "graphql", "backend"]
  matches: [] = 0/4 = 0.0 NO MATCH
```

**Files:**
- Add matching logic to Prometheus agent instructions

**Acceptance criteria:**
- Matches are case-insensitive
- Partial word matches not counted (e.g., "front" != "frontend")
- Returns top 3 matches maximum

### Task 3.3: Add delegation prompt

When a match is found, prompt user for delegation choice.

**Prompt format:**
```
I found a skill that may help with this request:

  frontend-design
  Capabilities: generate-component, design-page

Would you like to:
  A) Delegate to frontend-design (recommended)
  B) Proceed with Maestro's standard workflow
  C) Use both (Maestro plans, frontend-design executes)
```

**Files:**
- Modify `.claude/agents/prometheus.md`

**Acceptance criteria:**
- Only shown when match score > 0.5
- User can skip with "B" or just pressing enter
- Choice logged in draft file

---

## Phase 4: Delegation Mechanism

### Task 4.1: Define delegation protocol

Create standard interface for delegating to external skills.

**Delegation message format:**
```json
{
  "action": "delegate",
  "skill": "frontend-design",
  "request": "design a login page with email and password fields",
  "context": {
    "project_path": "/Users/user/my-project",
    "tech_stack": ["react", "tailwind"],
    "constraints": ["must match existing style"]
  },
  "callback": {
    "type": "file",
    "path": ".maestro/delegations/{id}.result.json"
  }
}
```

**Files:**
- Document protocol in `.claude/skills/maestro/references/delegation-protocol.md`

**Acceptance criteria:**
- Protocol is skill-agnostic
- Supports context passing
- Defines success/failure response format

### Task 4.2: Implement Task-based delegation

Use Claude Code's Task tool to invoke external skill agents.

**Delegation via Task:**
```
Task(
  description: "Design login page",
  subagent_type: "frontend-design",  # Maps to external skill
  prompt: "...",
  context: {...}
)
```

**Fallback for non-agent skills:**
- If skill has commands but no agents, use command invocation
- If skill has neither, log warning and fall back to Maestro

**Files:**
- Modify `.claude/agents/prometheus.md` - Add delegation logic
- Modify `.claude/agents/orchestrator.md` - Handle delegation results

**Acceptance criteria:**
- External agent invoked correctly
- Results captured and returned to Maestro
- Timeout handling (default 10 minutes)

### Task 4.3: Result integration

Integrate external skill results back into Maestro workflow.

**Result handling:**
1. External skill writes result to callback path
2. Maestro reads result
3. If success: continue with plan, note delegation in draft
4. If failure: offer retry or fallback to Maestro

**Files:**
- Modify `.claude/agents/orchestrator.md`

**Acceptance criteria:**
- Results parsed correctly
- Failures don't crash workflow
- User informed of delegation outcome

---

## Future: Marketplace Integration

(Not in current scope - documented for reference)

### Future Task: Marketplace discovery
- Add `source: "marketplace"` support
- Implement API client for Anthropic marketplace
- Cache marketplace listings locally

### Future Task: Skill installation
- `/install <skill-name>` command
- Download and validate skill packages
- Update registry automatically

### Future Task: Skill updates
- Version tracking in registry
- `/update` command for outdated skills
- Notification of available updates

---

## Verification

### Manual Testing
1. Install a test plugin (e.g., mock `frontend-design`)
2. Run `/discover` - verify it's found
3. Run `/design frontend page` - verify auto-detection
4. Choose delegation - verify skill is invoked
5. Check result integration

### Automated Checks
- `./scripts/validate-registry.sh` passes
- Discovery completes in < 2 seconds
- Registry schema is valid JSON

---

## Notes

### Technical Decisions
- File-based registry (not in-memory) for persistence across sessions
- Shell scripts for discovery (portable, no dependencies)
- Keyword matching threshold of 0.3 balances precision/recall

### Constraints
- Must not break existing Maestro workflows
- External skills are optional - Maestro works without them
- No new dependencies required

### Risks
- External skill may have incompatible interface - mitigate with protocol spec
- Discovery may be slow with many plugins - mitigate with caching
- Keyword matching may have false positives - mitigate with user confirmation
