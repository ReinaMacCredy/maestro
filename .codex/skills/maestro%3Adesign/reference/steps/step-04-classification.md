# Step 4: Project Classification

**Progress: Step 4 of 16** -- Next: Vision & Success Criteria

## Goal
Classify the project type, domain, and complexity level through collaborative discovery. This classification shapes which later steps are activated and how deep discovery goes.

## Execution Rules
- You MUST read `reference/classification-data.md` before starting this step.
- This is collaborative discovery -- you are a facilitator, not a form filler.
- NEVER skip straight to classification without conversation.
- ALWAYS confirm classification with the user before proceeding.

## Context Boundaries
- Product context from step 1 is available (product.md contents).
- Track description and type from step 2 are available.
- Classification data tables are loaded from reference file.
- Vision and success criteria are NOT yet defined (that is step 5).

## Discovery Sequence

1. **Load Classification Data**
   Read `reference/classification-data.md` to load project type and domain complexity tables.

2. **Analyze Available Context**
   Review what you know from product.md and the track description. Look for classification signals:
   - Project type signals (API keywords, UI keywords, platform references)
   - Domain signals (industry terms, compliance mentions, regulatory references)
   - Complexity indicators (regulated industry, novel technology, multi-system integration)

3. **Start Discovery Conversation**
   Share your initial read and ask clarifying questions:
   - "Based on the description and project context, this looks like a {type} project in the {domain} domain. Let me confirm a few things..."
   - Ask 2-3 targeted questions based on what is ambiguous.
   - If the project type is unclear, ask about the primary delivery mechanism (web, API, CLI, mobile, etc.).
   - If the domain is unclear, ask about the industry and any regulatory/compliance concerns.

4. **Confirm Classification**
   Present your classification for confirmation:
   ```
   --> Project Classification:
   - Project Type: {type}
   - Domain: {domain}
   - Complexity: {low | medium | high}

   This means:
   - Key concerns: {from classification data}
   - Compliance: {from classification data, or "None specific"}
   ```

   Let the user confirm or correct.

5. **Store Classification**
   Hold the confirmed classification in memory for use by subsequent steps. It influences:
   - Step 7 (Domain & Scoping): skipped if domain=general AND complexity=low
   - Step 9 (NFRs): compliance requirements seeded from classification
   - Step 11 (Codebase Scan): focus areas informed by project type

## Quality Checks
- [ok] Classification data loaded before conversation started
- [ok] At least 2 clarifying questions asked (not just auto-classified)
- [ok] User explicitly confirmed the classification
- [ok] All three dimensions classified: type, domain, complexity

## Anti-patterns
- [x] Auto-classifying without conversation -- always engage even when signals are strong
- [x] Asking the user to pick from the full table -- YOU analyze and propose, user confirms
- [x] Skipping complexity assessment -- this gates whether step 7 runs

## A/P/C Menu

Present: `[A] Advanced Elicitation  [P] Party Mode  [C] Continue to Vision & Success Criteria`

- **[A]**: Read `reference/elicitation-methods.md`. Suggest 3 methods most relevant to classification (e.g., First Principles for "like X but different" projects, Inversion for regulated domains). User picks one. Apply method to classification. Show improvements. User accepts or rejects. Redisplay menu.
- **[P]**: Read `reference/party-mode.md`. Cycle through all 5 perspectives on the classification. Present findings. User accepts or rejects improvements. Redisplay menu.
- **[C]**: Proceed to next step.

## Next Step
Read and follow `reference/steps/step-05-vision.md`.
