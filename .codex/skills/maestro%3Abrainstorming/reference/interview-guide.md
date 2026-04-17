# Brainstorming Interview Guide

Follow these questions in order. Skip any the user already answered in their initial request. Offer multiple-choice options where listed -- the user can pick one or give their own answer.

---

## 1. Problem Space (always ask)

**Q1: "What problem does this solve?"**
Options:
- User-facing pain point -- something end users hit
- Developer experience -- internal friction, bad ergonomics
- Performance/reliability gap -- something is too slow or breaks
- Missing capability -- can't do X at all today

**Q2: "How is this handled today?"**
Options:
- Not handled -- this is greenfield, nothing exists
- Manual workaround -- people do it by hand or hack around it
- Partial implementation -- something exists but is incomplete
- External tool/library -- a third-party solution is being replaced

If the user says "partial implementation" or "external tool", follow up:
- "What works about the current approach? What breaks?"

---

## 2. Scope (always ask)

**Q3: "What's the smallest version that would be useful?"**
(open-ended -- this defines MVP. Push back if the answer sounds like the full vision, not a slice.)

**Q4: "What's explicitly NOT part of this?"**
(open-ended -- forces non-goals. If the user struggles, suggest likely over-reaches based on the problem space.)

---

## 3. Users & Context (ask unless obvious)

**Q5: "Who uses this and when?"**
Options:
- End users -- via UI, API, or CLI
- Developers -- during development, CI, or debugging
- Operators -- monitoring, deployment, admin
- Automated -- triggered by other systems, no human in the loop

Skip if the problem space answer already made this clear.

---

## 4. Integration (ask if existing codebase is involved)

**Q6: "Which existing modules or systems does this touch?"**
(Auto-infer from a quick codebase scan if possible. Present findings and ask the user to confirm or correct.)

**Q7: "Are there APIs, data formats, or contracts this must conform to?"**
Options:
- Yes -- existing internal API/schema (ask which)
- Yes -- external/third-party API (ask which)
- No -- this is self-contained
- Not sure -- need to investigate

---

## 5. Constraints (ask what hasn't been covered)

Only ask constraints the user hasn't already mentioned. Pick the most relevant:

**Q8: "Any hard requirements?"**
Examples to prompt with (pick 1-2 relevant ones):
- Performance targets (latency, throughput)
- Backward compatibility
- Security/auth requirements
- Platform/browser/runtime constraints
- Data volume or storage limits

If the user says "nothing special", move on. Don't push for constraints that don't exist.

---

## 6. Risk (ask for ambitious or uncertain features)

Skip for simple, well-understood changes.

**Q9: "What's the hardest part of this?"**
Options:
- Data modeling -- getting the shape right
- Performance at scale -- making it fast enough
- Third-party integration -- depending on external systems
- UX complexity -- making it intuitive
- Uncertainty -- we don't fully understand the problem yet

**Q10: "What assumption, if wrong, would break this design?"**
(open-ended -- surfaces fragile foundations early)

---

## Interview Flow Rules

- **Skip answered questions**: If the user's initial message already covers a question, don't re-ask it. Acknowledge what you understood and move to the next gap.
- **Batch when natural**: If two questions are closely related (e.g., Q3 and Q4 about scope), you can ask them together. But never more than two.
- **Read the room**: If the user gives long, detailed answers, ask fewer questions. If answers are terse, probe deeper.
- **Stop when you have enough**: You don't need to ask every question. Once you can confidently sketch the design, move to the "Exploring approaches" phase. Typical interviews are 4-6 questions, not 10.
- **Summarize before designing**: After the interview, give a one-paragraph summary of what you understood. Ask "Does this capture it?" before presenting approaches.
