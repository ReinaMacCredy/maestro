---
name: maestro-design
description: "Deep discovery and specification for ambitious features. Full BMAD-inspired interview with classification, vision, journeys, domain analysis, and FR synthesis. Same output contract (spec.md + plan.md) as new-track but far richer. Use for multi-component systems, regulated domains, or unclear requirements."
argument-hint: "<track description>"
---

# Design -- Deep Discovery & Specification

Full-ceremony specification process for ambitious features. Produces the same `spec.md` + `plan.md` output as `/maestro:new-track` but through deep, multi-step discovery inspired by BMAD methodology.

**When to use this instead of `/maestro:new-track`:**
- Multi-component systems with many moving parts
- Regulated domains (healthcare, fintech, govtech)
- Unclear or complex requirements that need thorough discovery
- Features where getting the spec wrong is expensive

## Arguments

`$ARGUMENTS`

The track description. Examples: `"Add user authentication with OAuth and RBAC"`, `"Build real-time collaboration engine"`, `"Implement HIPAA-compliant patient portal"`

---

## Step Sequence

This skill uses a step-file architecture. Each step is a self-contained file in `reference/steps/`. You MUST load ONE step at a time, execute it fully, then load the NEXT. Previous step files get dropped from context.

**Rules:**
- NEVER load multiple step files simultaneously
- ALWAYS read the entire step file before executing
- NEVER skip steps or reorder the sequence
- Steps 4-9 include an A/P/C menu -- the user MUST select [C] before you proceed

### Step 1: Validate Prerequisites
Check product.md and tracks.md exist.
--> Read and follow `reference/steps/step-01-init.md`

### Step 2: Parse Input & Generate Track ID
Extract description, infer type, generate `{shortname}_{YYYYMMDD}` ID.
--> Read and follow `reference/steps/step-02-parse-input.md`

### Step 3: Create Track Directory
Create `.maestro/tracks/{track_id}/`.
--> Read and follow `reference/steps/step-03-create-dir.md`

### Step 4: Project Classification
Classify project type, domain, and complexity using `reference/classification-data.md`.
--> Read and follow `reference/steps/step-04-classification.md`

### Step 5: Vision & Success Criteria
Define vision, measurable success criteria, and MVP/Growth/Vision scope.
--> Read and follow `reference/steps/step-05-vision.md`

### Step 6: User Journey Mapping
Map ALL user types with narrative journeys. Minimum 3 journeys. Extract capability hints.
--> Read and follow `reference/steps/step-06-journeys.md`

### Step 7: Domain & Scoping
Domain-specific requirements and risk analysis. Skipped if domain=general AND complexity=low.
--> Read and follow `reference/steps/step-07-domain.md`

### Step 8: Functional Requirements Synthesis
Synthesize FRs from all discovery. Grouped by capability area. Each FR testable and implementation-agnostic.
--> Read and follow `reference/steps/step-08-functional.md`

### Step 9: Non-Functional Requirements
Performance, security, scalability, compatibility. Measurable format.
--> Read and follow `reference/steps/step-09-nonfunctional.md`

### Step 10: Spec Draft & Approval
Compose enriched spec from all discovery. Present for approval. Write spec.md.
--> Read and follow `reference/steps/step-10-spec-approval.md`

### Step 11: Codebase Pattern Scan
Scan codebase for existing patterns relevant to this track. Feed into plan context.
--> Read and follow `reference/steps/step-11-codebase-scan.md`

### Step 12: Implementation Plan with Traceability
Generate plan with FR traceability and coverage matrix. Present for approval. Write plan.md.
--> Read and follow `reference/steps/step-12-plan.md`

### Step 13: Pre-Implementation Readiness Gate
Validate FR coverage, AC coverage, dependency sanity, scope alignment. Pass/fail gate.
--> Read and follow `reference/steps/step-13-readiness.md`

### Step 14: Metadata, Registry, Commit & Summary
Write metadata.json, update tracks.md, commit, display summary.
--> Read and follow `reference/steps/step-14-commit.md`

---

## Relationship to Other Skills

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Lightweight track creation (use for simple, well-understood features)
- `/maestro:design` -- **You are here.** Deep discovery for ambitious features
- `/maestro:implement` -- Execute the implementation plan
- `/maestro:review` -- Verify implementation against spec
- `/maestro:status` -- Check progress across all tracks

A track created here produces `spec.md` and `plan.md` that `/maestro:implement` consumes. The enriched spec serves as the baseline for `/maestro:review`. Deep specs lead to better implementations -- invest in the discovery.
