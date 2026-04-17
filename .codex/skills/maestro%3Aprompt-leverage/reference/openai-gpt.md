# GPT Prompt Patterns

Distilled from OpenAI's official prompt guidance for GPT-5.4. Focuses on prompt-level patterns only -- no API config, SDK parameters, compaction, or migration guides.

Last synced: 2026-03-13 (GPT-5.4 era)

## Output Contracts

GPT-5.4 performs best with explicit output contracts that define what to return and how.

```xml
<output_contract>
- Return exactly the sections requested, in the requested order.
- Apply length limits only to the section they are intended for.
- If a format is required (JSON, Markdown, SQL, XML), output only that format.
</output_contract>
```

**Verbosity controls** (pair with output contracts):

```xml
<verbosity_controls>
- Prefer concise, information-dense writing.
- Avoid repeating the user's request.
- Keep progress updates brief.
- Do not shorten so aggressively that evidence, reasoning, or completion checks
  are omitted.
</verbosity_controls>
```

**Strict format clamping** (for parse-sensitive outputs like SQL, JSON):

```xml
<structured_output_contract>
- Output only the requested format.
- Do not add prose or markdown fences unless requested.
- Validate that parentheses and brackets are balanced.
- Do not invent tables or fields.
- If required schema information is missing, ask for it or return an error object.
</structured_output_contract>
```

## Follow-Through and Instruction Priority

**Default follow-through policy:**

```xml
<default_follow_through_policy>
- If the user's intent is clear and the next step is reversible and low-risk,
  proceed without asking.
- Ask permission only if the next step is:
  (a) irreversible,
  (b) has external side effects, or
  (c) requires missing sensitive information.
- If proceeding, briefly state what you did and what remains optional.
</default_follow_through_policy>
```

**Instruction priority:**

```xml
<instruction_priority>
- User instructions override default style, tone, formatting, and initiative.
- Safety, honesty, privacy, and permission constraints do not yield.
- Newer user instructions override older ones. Preserve non-conflicting earlier ones.
</instruction_priority>
```

**Mid-conversation updates** -- be explicit about scope, override, and carry-forward:

```xml
<task_update>
The task has changed.
Previous task: [what it was].
Current task: [what it is now].
Rules for this turn: [specific overrides].
All earlier instructions still apply unless they conflict with this update.
</task_update>
```

## Tool Use

**Tool persistence** -- keep calling tools until the task is actually done:

```xml
<tool_persistence_rules>
- Use tools whenever they materially improve correctness or completeness.
- Do not stop early when another call would materially improve the result.
- Keep calling tools until the task is complete AND verification passes.
- If a tool returns empty or partial results, retry with a different strategy.
</tool_persistence_rules>
```

**Dependency checks** -- GPT-5.4 can skip prerequisites when the end state seems obvious:

```xml
<dependency_checks>
- Before taking an action, check whether prerequisite lookup or retrieval is required.
- Do not skip prerequisite steps just because the intended final action seems obvious.
- If the task depends on the output of a prior step, resolve that dependency first.
</dependency_checks>
```

**Parallel tool calling:**

```xml
<parallel_tool_calling>
- When multiple retrieval steps are independent, prefer parallel calls.
- Do not parallelize steps with prerequisite dependencies.
- After parallel retrieval, pause to synthesize before making more calls.
- Prefer selective parallelism: parallelize evidence gathering, not speculative use.
</parallel_tool_calling>
```

## Completeness and Verification

**Completeness contract** -- prevents partial coverage and premature finish:

```xml
<completeness_contract>
- Treat the task as incomplete until all requested items are covered or marked [blocked].
- Keep an internal checklist of required deliverables.
- For lists, batches, or paginated results: determine scope, track processed items,
  confirm coverage before finalizing.
- If any item is blocked by missing data, mark it [blocked] and state what is missing.
</completeness_contract>
```

**Empty result recovery:**

```xml
<empty_result_recovery>
If a lookup returns empty, partial, or suspiciously narrow results:
- Do not immediately conclude that no results exist.
- Try at least 1-2 fallback strategies: alternate wording, broader filters,
  prerequisite lookup, or alternate source.
- Only then report that no results were found, along with what you tried.
</empty_result_recovery>
```

**Verification loop** (before finalizing or taking irreversible action):

```xml
<verification_loop>
Before finalizing:
- Correctness: does the output satisfy every requirement?
- Grounding: are factual claims backed by provided context or tool outputs?
- Formatting: does the output match the requested schema or style?
- Safety: if the next step has external side effects, ask permission first.
</verification_loop>
```

**Missing context gating:**

```xml
<missing_context_gating>
- If required context is missing, do NOT guess.
- Prefer lookup tools when the missing context is retrievable.
- Ask a clarifying question only when lookup is not possible.
- If you must proceed, label assumptions explicitly and choose a reversible action.
</missing_context_gating>
```

**Action safety frame** (for agents that take real actions):

```xml
<action_safety>
- Pre-flight: summarize intended action and parameters in 1-2 lines.
- Execute via tool.
- Post-flight: confirm outcome and any validation performed.
</action_safety>
```

## Research and Citations

**Structured research mode:**

```xml
<research_mode>
Do research in 3 passes:
1) Plan: list 3-6 sub-questions to answer.
2) Retrieve: search each sub-question and follow 1-2 second-order leads.
3) Synthesize: resolve contradictions and write the final answer with citations.
Stop only when more searching is unlikely to change the conclusion.
</research_mode>
```

**Citation and grounding rules:**

```xml
<citation_rules>
- Only cite sources retrieved in the current workflow.
- Never fabricate citations, URLs, IDs, or quote spans.
- Use exactly the citation format required by the host application.
- Attach citations to specific claims they support, not only at the end.
</citation_rules>

<grounding_rules>
- Base claims only on provided context or tool outputs.
- If sources conflict, state the conflict and attribute each side.
- If context is insufficient, narrow the answer or say you cannot support the claim.
- Label inferences as inferences, not as directly supported facts.
</grounding_rules>
```

## Coding and Agentic Patterns

**Autonomy and persistence:**

```xml
<autonomy_and_persistence>
Persist until the task is fully handled end-to-end. Do not stop at analysis or
partial fixes; carry changes through implementation, verification, and explanation.
Unless the user explicitly asks for a plan or is brainstorming, assume they want
you to make code changes. If you encounter blockers, attempt to resolve them yourself.
</autonomy_and_persistence>
```

**User updates** -- brief, outcome-based, not narrating every tool call:

```xml
<user_updates_spec>
- Only update the user when starting a new major phase or when the plan changes.
- Each update: 1 sentence on outcome + 1 sentence on next step.
- Do not narrate routine tool calls.
- Keep user-facing status short; keep the work exhaustive.
</user_updates_spec>
```

**Initiative nudge** (when the model stops at first plausible answer):

```xml
<dig_deeper_nudge>
- Don't stop at the first plausible answer.
- Look for second-order issues, edge cases, and missing constraints.
- If the task is safety or accuracy critical, perform at least one verification step.
</dig_deeper_nudge>
```

## Reasoning Model Calibration (o-series vs. gpt-series)

Not all tasks need reasoning models. Match model choice to task shape.

### When to Use o-series (o1, o3, o4-mini)

- Complex multi-step logic (math, algorithms, proofs)
- Tasks where chain-of-thought visibly improves accuracy
- Planning with many interdependent constraints
- Code that requires understanding complex state machines

### When gpt-series Is Sufficient

- Straightforward coding (add field, fix typo, rename variable)
- Text generation and editing
- Data extraction and formatting
- Most tool-use workflows

### Prompting Differences

**gpt-series** responds well to:
- Detailed step-by-step instructions
- Examples and few-shot demonstrations
- XML-tagged structural blocks

**o-series** responds well to:
- Clear problem statement without over-specifying the approach
- Constraints and success criteria (let the model reason about HOW)
- Minimal few-shot examples (reasoning models derive approach from the problem)

```
-- GPT-series style (good):
   "Step 1: Read the config file. Step 2: Extract the database URL.
   Step 3: Parse host, port, and database name. Step 4: Return as object."

-- o-series style (good):
   "Parse database connection strings into { host, port, database } objects.
   Handle: standard postgres URLs, socket paths, and URLs with query parameters.
   Return null for unparseable strings."
```

**Anti-pattern:** Using o-series with step-by-step instructions. The reasoning model may follow your steps even when it would find a better approach on its own.

## Before/After: GPT Prompt Transformation

### Raw Prompt (Weak)

```
Write me an API endpoint that handles file uploads
```

### Upgraded Prompt (Strong)

```
## Objective
Add a POST /api/files/upload endpoint to the Express API.
Accept multipart/form-data with a single file field named "document".

## Constraints
- Max file size: 10MB. Return 413 if exceeded.
- Allowed types: .pdf, .docx, .txt. Return 415 for others.
- Store files in uploads/ directory with UUID filenames (preserve extension).
- Return { id: string, originalName: string, size: number, uploadedAt: string }.

## Context
- Existing routes follow the pattern in src/api/routes/users.ts
- File utilities exist in src/utils/file.ts (mime type detection, path sanitization)
- No existing upload handling anywhere in the codebase

<output_contract>
- Return the complete route handler file.
- Include input validation, error responses, and success response.
- Add tests covering: valid upload, oversized file, wrong type, missing file field.
</output_contract>

<verification_loop>
Before finalizing:
- All error cases return appropriate HTTP status codes
- File is written to disk only after all validation passes
- UUID generation uses crypto.randomUUID() (not Math.random())
- Test file is self-contained (creates and cleans up test files)
</verification_loop>
```

## Frontend Patterns

```xml
<frontend_tasks>
When doing frontend design tasks, avoid generic, overbuilt layouts.
- First viewport reads as one composition, not a dashboard (unless it is one).
- Brand or product name must be hero-level signal, not just nav text.
- Default to no cards. Only use when they serve interaction or understanding.
- One job per section: one purpose, one headline, one short supporting sentence.
- Use motion for presence and hierarchy, not noise. 2-3 intentional motions.
- Reduce clutter: avoid pill clusters, stat strips, icon rows, competing text blocks.
</frontend_tasks>
```

## GPT-Specific Pitfalls

| Pitfall | What Happens | Fix |
|---------|-------------|-----|
| No output contract | GPT picks its own format, often verbose | Add `<output_contract>` with explicit format |
| Missing completeness contract | GPT finishes early on multi-item tasks | Add `<completeness_contract>` with tracking |
| Step-by-step for o-series | Model follows suboptimal steps instead of reasoning | State the goal and constraints, let it reason |
| No empty-result recovery | GPT reports "not found" on first failure | Add `<empty_result_recovery>` with fallback strategies |
| Vague tool instructions | GPT uses tools speculatively or skips them | Add `<tool_persistence_rules>` with clear criteria |
| No verification loop | Code changes ship without self-review | Add `<verification_loop>` before finalization |
