# Plan: Documentation Rewrite

## Epic 1: README.md Rewrite
- [ ] E1.1: Write hero section with tagline + badges
- [ ] E1.2: Write install section (CC+Amp primary, others collapsed)
- [ ] E1.3: Create Quick Start mermaid diagram
- [ ] E1.4: Write skills table (9 skills, correct names)
- [ ] E1.5: Add doc links section
- [ ] E1.6: Review and finalize README

## Epic 2: SETUP_GUIDE.md Rewrite
- [ ] E2.1: Write Claude Code install section
- [ ] E2.2: Write Amp install section
- [ ] E2.3: Write other agents reference section
- [ ] E2.4: Update global config block to current version
- [ ] E2.5: Fix bd CLI install command (correct source)
- [ ] E2.6: Write Agent Mail MCP setup
- [ ] E2.7: Add MCPorter toolboxes section
- [ ] E2.8: Write verification checklist
- [ ] E2.9: Review and finalize SETUP_GUIDE

## Epic 3: TUTORIAL.md Rewrite
- [ ] E3.1: Write "Why Maestro" section
- [ ] E3.2: Write core concepts section
- [ ] E3.3: Write 10-phase pipeline walkthrough
- [ ] E3.4: Write research hooks explanation
- [ ] E3.5: Write A/P/C checkpoints section
- [ ] E3.6: Write multi-session example with handoffs
- [ ] E3.7: Add BMAD/Oracle behavior summary
- [ ] E3.8: Write troubleshooting section
- [ ] E3.9: Review and finalize TUTORIAL

## Epic 4: REFERENCE.md Rewrite
- [ ] E4.1: Write full commands table
- [ ] E4.2: Write full triggers table
- [ ] E4.3: Write skills reference (9 skills)
- [ ] E4.4: Write bd CLI cheatsheet
- [ ] E4.5: Write 10-phase pipeline detail table
- [ ] E4.6: Write A/P/C state ladder and validation gates
- [ ] E4.7: Write directory structure section
- [ ] E4.8: Write fallback/HALT/DEGRADE policies
- [ ] E4.9: Add skill authoring pointers
- [ ] E4.10: Add MCPorter toolboxes reference
- [ ] E4.11: Review and finalize REFERENCE

## Epic 5: Final Integration
- [ ] E5.1: Verify all internal links work
- [ ] E5.2: Cross-check consistency across all docs
- [ ] E5.3: Update AGENTS.md config block if needed
- [ ] E5.4: Final review and commit

## Dependencies
- E2 depends on E4 (SETUP links to REFERENCE)
- E3 depends on E4 (TUTORIAL links to REFERENCE)
- E5 depends on E1, E2, E3, E4
