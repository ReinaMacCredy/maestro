# Synthesis Guide -- Merging Scout Results

## The Synthesis Problem

Scouts return independent findings. Your job is to turn fragments into a coherent picture. This is the hardest part of parallel exploration -- raw results are not the answer.

## Synthesis Steps

### 1. Deduplicate

Scouts working in adjacent domains will mention the same files. This is expected.

**Process:**
1. List every file path mentioned across all scout results
2. Note which scouts mentioned each file and why
3. Files mentioned by 2+ scouts are integration points -- flag them for closer reading

**Example:**

| File | Scout 1 (Structure) | Scout 2 (Data flow) | Scout 3 (Config) |
|------|-------------------|-------------------|-----------------|
| `registry.ts` | Exports `loadSkill()` | Called by `cli.ts` in load chain | Reads `SKILL_PATH` env var |
| `loader.ts` | Exports `parseSkillFile()` | Transforms raw YAML to SkillDef | -- |
| `config.ts` | -- | -- | Defines all defaults |

The registry is the integration point (3/3 scouts). Loader is implementation detail (2/3). Config is isolated (1/3).

### 2. Resolve Contradictions

Scouts can return contradictory information. This happens when:
- Different parts of the codebase do the same thing differently (real inconsistency)
- A scout misread the code (scout error)
- The codebase has dead code or legacy paths (noise)

**Resolution protocol:**

| Contradiction type | Action |
|-------------------|--------|
| Two implementations of the same thing | Both are real. Document both. Flag as tech debt or intentional (check git blame for context). |
| Scout says X, another says not-X | Re-read the specific code yourself. One scout is wrong. |
| Scout found something that seems outdated | Check if the code is reachable. Dead code is noise -- note it but don't build on it. |

**Never average contradictions.** "Scout 1 says retries=3, Scout 2 says retries=5" does not mean retries=4. Find the truth.

### 3. Identify Gaps

After deduplication, check coverage:

**Gap checklist:**
- [ ] Do I understand the entry point? (How does the operation start?)
- [ ] Do I understand the exit point? (How does the operation end? What's returned?)
- [ ] Do I understand error paths? (What happens when it fails?)
- [ ] Do I understand configuration? (What knobs exist?)
- [ ] Do I understand test coverage? (Is this well-tested or fragile?)

Gaps that matter for your task need a follow-up scout or manual investigation. Gaps that don't matter should be noted but not pursued.

### 4. Build the Summary

Structure your synthesis as a **narrative with evidence**, not a list of scout outputs.

**Bad synthesis (scout dump):**
```
Scout 1 found: registry.ts, loader.ts, types.ts
Scout 2 found: cli.ts calls loadSkill(), loadSkill() calls parseFile()
Scout 3 found: SKILL_PATH env var, default is ./skills/
```

**Good synthesis (integrated narrative):**
```
Skill loading is a 3-step pipeline:

1. CLI entry (cli.ts:45) parses the skill name from argv
2. Registry lookup (registry.ts:23) resolves name to file path
   - Uses SKILL_PATH env var (default: ./skills/)
   - Falls back to built-in directory if not found
3. File parsing (loader.ts:67) reads YAML frontmatter + markdown body
   - Returns SkillDef { name, description, content }
   - Throws SkillNotFoundError if file missing

Integration point: registry.ts is the hub -- both CLI and loader
depend on it. Changes here affect both the lookup and the fallback path.

Gap: No tests found for the fallback path. loader.test.ts only
tests the happy path with explicit file paths.
```

## Prioritizing Discoveries

Not all findings are equally important. Prioritize by relevance to your task.

**Priority framework:**

| Priority | Criteria | Action |
|----------|----------|--------|
| P0 -- Blocking | Directly answers the original question | Include in synthesis, cite evidence |
| P1 -- Important | Affects design decisions for the task | Include in synthesis, note implications |
| P2 -- Context | Good to know but doesn't change the plan | Mention briefly, don't elaborate |
| P3 -- Noise | Interesting but irrelevant | Omit from synthesis entirely |

**Common mistake:** Treating all findings as P0. A scout that found 15 interesting things about the auth system when you're investigating the skill loader produced P3 noise. Don't let thoroughness dilute the signal.

## Synthesis Anti-Patterns

| Anti-pattern | Problem | Fix |
|-------------|---------|-----|
| Scout dump | Paste each scout's output with no integration | Write a narrative that cross-references findings |
| Lost contradictions | Gloss over conflicting findings | Resolve explicitly: which is correct and why |
| Completionism | Report everything found, regardless of relevance | Prioritize. Omit P3 findings. |
| Gap blindness | Report what was found, ignore what wasn't | Run the gap checklist. Missing answers matter. |
| Second-hand evidence | Report scout claims without verifying critical facts | Re-read integration points yourself for P0 findings |

## When Synthesis Reveals You Need More

Sometimes synthesis shows the investigation was incomplete. This is normal.

**Decision tree:**

```
Synthesis complete?
  |
  +-- Yes, answers the original question --> Done. Report.
  |
  +-- No, specific gap identified
       |
       +-- Gap is narrow (one file, one function) --> Read it yourself. No scout needed.
       |
       +-- Gap is broad (new domain, multiple files) --> Launch 1-2 targeted follow-up scouts.
       |
       +-- Gap is fundamental (wrong question asked) --> Reframe the question. New round of 2-3 scouts.
```

**Limit follow-up rounds to 2.** If two rounds of scouts haven't answered your question, you're either asking the wrong question or the answer requires reading the code yourself. See the "Diminishing Returns" section in SKILL.md.
