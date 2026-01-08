# Spec: Documentation Rewrite

## Requirements

### README.md
- [ ] Hero section with tagline + badges
- [ ] Install commands for Claude Code + Amp (primary)
- [ ] Collapsible section for other agents
- [ ] Quick Start diagram (ds → ci → finish)
- [ ] Skills table with 9 current skills (correct gerund names)
- [ ] Links to other docs

### SETUP_GUIDE.md
- [ ] Detailed install for Claude Code + Amp
- [ ] Reference install for Codex, Cursor, Gemini CLI
- [ ] Current global config block (version 2.2.0+)
- [ ] bd CLI install from correct source
- [ ] Agent Mail MCP setup
- [ ] MCPorter toolboxes explanation
- [ ] Verification checklist

### TUTORIAL.md
- [ ] "Why Maestro" section
- [ ] Core concepts: Conductor, Beads, Skills, Tracks
- [ ] 10-phase unified pipeline walkthrough
- [ ] Research hooks (2, not 5)
- [ ] A/P/C checkpoints explanation
- [ ] Multi-session example with handoffs
- [ ] BMAD/Oracle behavior summary
- [ ] Troubleshooting tips

### REFERENCE.md
- [ ] Full commands table
- [ ] Full triggers table
- [ ] Skills reference (9 skills with descriptions)
- [ ] bd CLI cheatsheet
- [ ] 10-phase pipeline detail table
- [ ] A/P/C state ladder
- [ ] Validation gates
- [ ] Directory structure
- [ ] Fallback/HALT/DEGRADE policies
- [ ] Skill authoring pointers
- [ ] MCPorter toolboxes reference

## Acceptance Criteria
1. All skill names use gerund form (designing, tracking, creating-skills)
2. Pipeline described as 10-phase unified
3. maestro-core described as routing shim, not central orchestrator
4. Claude Code + Amp setup is copy-pasteable
5. No redundant content between docs (REFERENCE is canonical for tables)
6. All internal links work
