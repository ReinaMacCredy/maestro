---
candidate_only: true
runtime_activation: false
runtime_registration: false
logical_resource: skills/maestro/jobs/review.md
---
# Review job

Resolve exactly one of Inspect, Audit, AdversarialReview, QAReplay, or
CloseReview, then load only its exact primary and admitted auxiliary subset.
Audit alone admits ArchitectureReview, GenerateAndFilter, and Verification;
CloseReview admits only Verification; the other modes admit no auxiliary.
The invocation result is exactly Produced, semantic Refused, or conclusive
Failed. Produced carries the mode-specific result plus a complete disjoint
coverage partition and provenance. A pre-route refusal creates no invocation;
crash, transport failure, or effect uncertainty creates no result envelope.
