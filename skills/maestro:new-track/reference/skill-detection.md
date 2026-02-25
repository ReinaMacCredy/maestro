# Skill Detection Protocol

After generating the implementation plan, identify installed skills that could provide domain expertise during implementation.

---

## 9.5.1: Check Learning Cache

Read `.maestro/context/skill-mappings.md` (if exists).

If a mapping row exists where:
- Track type matches, AND
- 2+ keywords from the row appear in this track's description

Then use the cached skill names directly. Mark each with `relevance: "cached"`. Skip to Step 9.5.4.

If no cache hit, proceed to Step 9.5.2.

## 9.5.2: Build Match Corpus

Combine these into a single text corpus for matching:
- Track description (from Step 2)
- Track type (feature/bug/chore)
- Technology keywords from `.maestro/context/tech-stack.md`
- Phase titles and task titles from the generated plan

## 9.5.3: Match Skills

The runtime injects a list of all installed skills (names + descriptions) into the agent's context at conversation start. Use this list as the skill registry.

For each skill in the runtime skill list:
1. Check if the skill's description keywords overlap with the match corpus
2. Prioritize skills whose description explicitly mentions technologies, frameworks, or patterns present in the match corpus
3. Exclude skills that are clearly irrelevant (e.g., `reset`, `status`, `pipeline` -- workflow/utility skills, not domain expertise)

**Relevance filter**: Only match skills that provide domain expertise (coding patterns, framework guidance, testing strategies). Skip workflow/orchestration skills.

## 9.5.4: Record Matched Skills

If skills were matched, store them for metadata.json.

Print an informational message:

```
[ok] Detected {N} relevant skill(s) for this track:
  --> {skill-1-name}: {one-line description}
  --> {skill-2-name}: {one-line description}
These will be auto-loaded during /maestro:implement.
```

If no skills matched, print nothing.
