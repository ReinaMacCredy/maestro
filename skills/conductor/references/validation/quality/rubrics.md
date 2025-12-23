# Compact Summary Scoring Rubrics

Scoring rubrics for evaluating compaction quality across 6 dimensions. Each dimension is scored 0-5.

---

## 1. Accuracy

**What it measures:** Factual correctness of file paths, technical details, code snippets, command outputs, and stated conclusions.

| Score | Criteria |
|-------|----------|
| **0** | Multiple factual errors: wrong file paths, incorrect function names, fabricated technical details, or statements contradicted by conversation evidence. |
| **3** | Core facts correct but minor inaccuracies: slightly wrong line numbers, incomplete paths that are still resolvable, or paraphrased technical details that lose precision. |
| **5** | All file paths, function names, error messages, and technical details match conversation evidence exactly. No invented or hallucinated information. |

**Judge guidance:** Cross-reference every specific claim (paths, names, values) against the original conversation. Flag any discrepancy.

---

## 2. Context Awareness

**What it measures:** Understanding of current conversation state, what the user is trying to accomplish, and the state of artifacts being worked on.

| Score | Criteria |
|-------|----------|
| **0** | Summary describes work as complete when it's in progress, misidentifies the current goal, or shows no awareness of what the user was actually doing. |
| **3** | Captures the general goal but misses nuance: e.g., knows user is "fixing tests" but not which specific test or why it matters. Current state described but vaguely. |
| **5** | Precisely identifies: (1) what user is trying to accomplish, (2) where they are in that process, (3) what decision or action comes next. Captures the "why" behind current work. |

**Judge guidance:** Ask "Could someone resume this conversation and immediately know what to do next?" Score based on that answer.

---

## 3. Artifact Trail Integrity

**What it measures:** Tracking of files created, modified, deleted, and their key details (purpose, location, relationships).

| Score | Criteria |
|-------|----------|
| **0** | Missing files that were created/modified, wrong paths, or no mention of artifact changes despite significant file work in conversation. |
| **3** | Lists most artifacts but missing some: e.g., mentions main file but not its test file, or lists files without explaining what changed in each. |
| **5** | Complete inventory of all files touched with: (1) full correct paths, (2) what was done (created/modified/deleted), (3) key content or purpose. Relationships between files noted. |

**Judge guidance:** Count artifacts mentioned in conversation vs. artifacts captured in summary. Check paths character-by-character.

---

## 4. Continuity Preservation

**What it measures:** Work state, open TODOs, unresolved questions, and reasoning chains that must survive compaction.

| Score | Criteria |
|-------|----------|
| **0** | Loses critical work state: forgets pending TODOs, drops unresolved blockers, or omits reasoning that explains why current approach was chosen. |
| **3** | Captures major TODOs but loses some context: e.g., lists "need to fix auth" but not the specific approach discussed, or preserves what but not why. |
| **5** | Preserves: (1) all explicit TODOs with their context, (2) open questions/blockers, (3) key decisions and their reasoning, (4) any "remember this for later" moments. Work can resume without re-discovery. |

**Judge guidance:** Identify every TODO, blocker, decision point, and "note to self" in conversation. Verify each appears in summary with sufficient context.

---

## 5. Completeness

**What it measures:** Whether all significant parts of the conversation are addressed with sufficient depth.

| Score | Criteria |
|-------|----------|
| **0** | Major conversation segments ignored: entire features discussed but not mentioned, or summary covers only the last few messages. |
| **3** | All major topics mentioned but some treated superficially: e.g., "also discussed caching" with no detail on what was decided or done. |
| **5** | Every significant topic covered proportionally: more detail for complex/important work, brief mention for minor tangents. Nothing important omitted. |

**Judge guidance:** Segment conversation into logical chunks (topics/tasks). Verify each chunk has proportional representation. Weight by importance, not just length.

---

## 6. Instruction Following

**What it measures:** Adherence to format requirements, length constraints, and structural conventions.

| Score | Criteria |
|-------|----------|
| **0** | Wrong format entirely (prose when structured requested), exceeds length limits significantly, or ignores explicit formatting instructions. |
| **3** | Correct format with minor deviations: slightly over length, missing one required section, or inconsistent heading levels. |
| **5** | Exact format match: all required sections present, within length limits, proper structure, consistent styling throughout. |

**Judge guidance:** Check against format spec mechanically. Count sections, measure length, verify structure. This dimension is objective.

---

## Aggregate Scoring

**Total possible:** 30 points

| Range | Quality Level |
|-------|---------------|
| 25-30 | Excellent - Summary fully preserves conversation for seamless continuation |
| 18-24 | Good - Minor gaps but work can resume with some context rebuilding |
| 12-17 | Adequate - Core preserved but significant context loss |
| 0-11 | Poor - Summary inadequate for continuation; re-read conversation needed |

**Minimum threshold for acceptance:** 18 points with no dimension scoring 0.
