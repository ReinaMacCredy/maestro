# Setup Workflow

## Purpose
Initialize a new or existing project with the Conductor methodology, creating all necessary scaffolding and configuration files.

## Prerequisites
- Git installed and available
- Write access to the project directory
- User available for interactive input

## State Management

### State File
`conductor/setup_state.json`

### State Values
| Value | Description | Next Step |
|-------|-------------|-----------|
| `""` (empty) | Fresh start | Begin setup |
| `"2.1_product_guide"` | Product guide complete | Section 2.2 |
| `"2.2_product_guidelines"` | Guidelines complete | Section 2.3 |
| `"2.3_tech_stack"` | Tech stack complete | Section 2.4 |
| `"2.4_code_styleguides"` | Styleguides complete | Section 2.5 |
| `"2.5_workflow"` | Workflow complete | Phase 6.0 |
| `"3.3_initial_track_generated"` | Track generated | Phase 7.0 |
| `"4.1_codemaps_generated"` | CODEMAPS complete | Phase 8.0 |
| `"5.1_continuity_configured"` | Setup complete | Halt |

## Workflow Steps

### Phase 1: Resume Check

1. **Read State File**
   - Check for `conductor/setup_state.json`
   - If not exists: new project, proceed to 1.2
   - If exists: resume from `last_successful_step`

2. **Resume Logic**
   - Parse `last_successful_step` value
   - Announce resume point to user
   - Jump to appropriate section
   - If `3.3_initial_track_generated`: Setup complete, halt

### Phase 2: Pre-initialization

1. **Welcome Message**
   - Explain the setup process overview
   - List steps: Discovery → Definition → Configuration → Track Generation

### Phase 3: Project Detection

1. **Classify Project Type**
   - **Brownfield Indicators**:
     - `.git`, `.svn`, `.hg` directories
     - Dependency files: `package.json`, `pom.xml`, `requirements.txt`, `go.mod`
     - Source directories: `src/`, `app/`, `lib/`
   - **Greenfield**: None of the above, empty or minimal docs only

2. **Execute Based on Type**
   - **Brownfield**: 
     - Request read-only scan permission
     - Analyze codebase (respect `.gitignore`, `.geminiignore`)
     - Extract tech stack from manifest files
   - **Greenfield**:
     - Initialize git if needed
     - Ask: "What do you want to build?"
     - Create `conductor/` directory
     - Initialize state file
     - Write initial concept to `product.md`

### Phase 4: Interactive Document Generation

For each document, follow the Interactive Generation Protocol:

#### 4.1 Product Guide (`product.md`)
- **Max Questions**: 5
- **Topics**: Target users, goals, features
- **State on Complete**: `"2.1_product_guide"`

#### 4.2 Product Guidelines (`product-guidelines.md`)
- **Max Questions**: 5
- **Topics**: Prose style, brand messaging, visual identity
- **State on Complete**: `"2.2_product_guidelines"`

#### 4.3 Tech Stack (`tech-stack.md`)
- **Max Questions**: 5
- **Topics**: Languages, frameworks, databases
- **Brownfield**: Confirm detected stack, don't propose changes
- **State on Complete**: `"2.3_tech_stack"`

### Phase 5: Guide Selection

1. **Code Styleguides**
   - List available guides from templates
   - Recommend based on tech stack
   - Copy selected guides to `conductor/code_styleguides/`
   - **State on Complete**: `"2.4_code_styleguides"`

2. **Workflow Configuration**
   - Copy default `workflow.md` template
   - Offer customization:
     - Test coverage threshold (default: 80%)
     - Commit frequency (task vs phase)
     - Summary method (git notes vs commit message)
   - **State on Complete**: `"2.5_workflow"`

### Phase 6: Initial Track Generation

1. **Requirements Gathering** (Greenfield only)
   - Max 5 questions about user stories, requirements
   - Use auto-generate option if user prefers

2. **Track Proposal**
   - Generate single initial track title
   - Greenfield: Usually MVP
   - Brownfield: Maintenance or enhancement focus
   - Get user approval

3. **Create Artifacts**
   - Generate `tracks.md` with first track entry
   - Create track directory: `conductor/tracks/<track_id>/`
   - Generate:
     - `metadata.json`
     - `spec.md`
     - `plan.md` (with phase verification tasks)
   - **State on Complete**: `"3.3_initial_track_generated"`

4. **Finalize**
   - Commit all files: `conductor(setup): Add conductor setup files`
   - Announce next steps: `/conductor:implement`

### Phase 8: Continuity Setup

Set up session state preservation for cross-session context.

1. **Detect Platform**
   - **Claude Code**: Check for `~/.claude/` directory
   - **Amp Code**: Check for `~/.config/amp/` directory
   - **Codex**: Neither of the above

2. **Create Sessions Structure**
   - Create `conductor/sessions/active/`
   - Create `conductor/sessions/archive/`
   - Create `conductor/.cache/`
   - Add `.gitkeep` files to empty directories
   - Create `conductor/sessions/.gitignore`:
     ```
     # Session state is personal, not shared
     active/LEDGER.md
     ```
   - Create `conductor/.cache/.gitignore`:
     ```
     # Cache files are regenerable
     *
     !.gitignore
     ```

3. **Platform-Specific Setup**
   - **Claude Code**:
     - Check if hooks exist at `~/.claude/hooks/dist/continuity.js`
     - If not, prompt: "Run `./scripts/install-global-hooks.sh` for auto context loading"
   - **Amp Code**:
     - Check if `~/.config/amp/AGENTS.md` exists
     - Prompt: "Add Continuity Protocol to ~/.config/amp/AGENTS.md (see skills/continuity/references/amp-setup.md)"
   - **Codex**:
     - Display: "Use `continuity load/save/handoff` commands manually"

4. **Update State**
   - **State on Complete**: `"5.1_continuity_configured"`

### Phase 7: CODEMAPS Generation

Generate architecture documentation to help agents orient quickly.

1. **Check for Existing CODEMAPS**
   - If `conductor/CODEMAPS/` exists:
     - Prompt: "CODEMAPS exists. Regenerate? [Y/n]"
     - If no, skip to Finalize
   - If not exists, proceed

2. **Detect Project Structure**
   - **Monorepo Detection**:
     - Check for `packages/`, `apps/` directories
     - Check `package.json` for `workspaces` field
     - If monorepo: generate per-package codemaps
   - **Standard Project**: generate single overview + module codemaps

3. **Generate CODEMAPS**
   - Create `conductor/CODEMAPS/` directory
   - **Always Generate**: `overview.md` with:
     - Project summary (from product.md)
     - Directory structure (top 2 levels only)
     - Key entry points
     - Data flow diagram (Mermaid)
   - **Conditionally Generate**: Module codemaps for significant areas:
     - Scan for: `src/`, `api/`, `lib/`, `skills/`, `components/`, `services/`
     - Create `[module].md` for each significant area
     - Use templates from `skills/conductor/references/CODEMAPS_TEMPLATE.md`
   - **Scale Limits**:
     - Directory depth: Top 2 levels only
     - Key files per codemap: Max 50 files
     - Module codemaps: Max 10 files
   - **Monorepo Mode**:
     - Create `overview.md` at root
     - Create `[package-name].md` for each package

4. **Create Metadata**
   - Generate `conductor/CODEMAPS/.meta.json`:
     ```json
     {
       "generated": "<ISO timestamp>",
       "generator": "/conductor-setup",
       "project_type": "<detected type>",
       "files": {
         "overview.md": { "generated": true, "user_modified": false },
         "[module].md": { "generated": true, "user_modified": false }
       }
     }
     ```

5. **Announce**
   - Display: "✅ Generated CODEMAPS: overview.md, [module codemaps...]"
   - **State on Complete**: `"4.1_codemaps_generated"`

## Interactive Generation Protocol

Used for all document generation phases:

1. **Classify Question Type**
   - **Additive**: Multiple answers allowed, add "(Select all that apply)"
   - **Exclusive Choice**: Single answer required

2. **Question Format**
   ```
   A) [Option A]
   B) [Option B]
   C) [Option C]
   D) [Type your own answer]
   E) [Autogenerate and review]
   ```

3. **Process Answers**
   - Source of truth: user's selected answers only
   - Ignore unselected options
   - Expand on choices for comprehensive output

4. **Confirmation Loop**
   ```
   A) Approve: Proceed
   B) Suggest Changes: Modify
   ```

5. **State Commit**
   - Write to state file after each successful section

## Error Handling

| Error | Action |
|-------|--------|
| Tool call fails | Halt, announce failure, await instructions |
| State file corrupted | Announce error, suggest re-running setup |
| Permission denied | Halt, explain required permissions |
| Git not initialized | Initialize automatically (Greenfield) or prompt (Brownfield) |

## Output Artifacts

```
conductor/
├── setup_state.json
├── product.md
├── product-guidelines.md
├── tech-stack.md
├── workflow.md
├── tracks.md
├── code_styleguides/
│   └── [selected guides].md
├── CODEMAPS/
│   ├── .meta.json
│   ├── overview.md
│   └── [module].md
└── tracks/
    └── <track_id>/
        ├── metadata.json
        ├── spec.md
        └── plan.md
```
