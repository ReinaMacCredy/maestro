# Plan: GitHub Actions for Changelog CI/CD

**Track ID:** changelog-cicd  
**Spec:** [spec.md](./spec.md)  
**Design:** [design.md](./design.md)

---

## Phase 1: Core Infrastructure

### Epic 1.1: git-cliff Configuration

**Goal:** Set up git-cliff with conventional commit parsing

#### Tasks

- [ ] **1.1.1** Create `cliff.toml` at repository root
  - Configure conventional commit parsing
  - Map: feat→Added, fix→Fixed, refactor→Changed, docs→Documentation
  - Skip chore commits
  - Use Keep a Changelog format
  - **File:** `cliff.toml`

- [ ] **1.1.2** Test git-cliff locally
  - Run `git-cliff --dry-run` to verify output
  - Ensure commits are parsed correctly
  - **Verify:** Output matches expected format

---

## Phase 2: Release Workflow

### Epic 2.1: Main Release Automation

**Goal:** Automate changelog + version bump on push to main

#### Tasks

- [ ] **2.1.1** Create `.github/workflows/` directory structure
  - **File:** `.github/workflows/` (directory)

- [ ] **2.1.2** Create release.yml workflow
  - Trigger: push to main
  - Permissions: contents: write
  - Concurrency: release group
  - Skip if commit contains `[skip ci]`
  - **File:** `.github/workflows/release.yml`

- [ ] **2.1.3** Add version bump detection step
  - Get commits since last tag
  - Fallback to v0.0.0 if no tags
  - Detect: BREAKING→major, feat→minor, fix→patch, else→none
  - **File:** `.github/workflows/release.yml`

- [ ] **2.1.4** Add version bump execution step
  - Parse current version from plugin.json
  - Calculate new version
  - Update plugin.json (`.version`)
  - Update marketplace.json (`.plugins[0].version`)
  - Export NEW_VERSION to env
  - **File:** `.github/workflows/release.yml`

- [ ] **2.1.5** Add changelog generation step
  - Install git-cliff
  - Run git-cliff to update CHANGELOG.md
  - **File:** `.github/workflows/release.yml`

- [ ] **2.1.6** Add commit, tag, and push step
  - Git config for bot user
  - Add changed files
  - Commit with `[skip ci]` marker
  - Create version tag
  - Push commit and tag
  - **File:** `.github/workflows/release.yml`

- [ ] **2.1.7** Add GitHub Release creation step
  - Use softprops/action-gh-release
  - Set tag name from NEW_VERSION
  - Include changelog in body
  - **File:** `.github/workflows/release.yml`

---

## Phase 3: PR Validation

### Epic 3.1: Version Sync Validation

**Goal:** Prevent version drift between JSON files

#### Tasks

- [ ] **3.1.1** Create validate.yml workflow
  - Trigger: pull_request to main
  - **File:** `.github/workflows/validate.yml`

- [ ] **3.1.2** Add version sync check step
  - Extract version from plugin.json
  - Extract version from marketplace.json (`.plugins[0].version`)
  - Compare and fail if mismatch
  - **File:** `.github/workflows/validate.yml`

- [ ] **3.1.3** Add changelog preview step
  - Install git-cliff
  - Generate preview of unreleased changes
  - Output to GITHUB_STEP_SUMMARY
  - **File:** `.github/workflows/validate.yml`

---

## Phase 4: Documentation & Setup

### Epic 4.1: Documentation Updates

**Goal:** Update docs to reflect automated workflow

#### Tasks

- [ ] **4.1.1** Update AGENTS.md versioning section
  - Document that plugin version is auto-bumped
  - Clarify skill versions remain manual
  - Add `[skip ci]` escape hatch info
  - **File:** `AGENTS.md`

- [ ] **4.1.2** Create initial version tag
  - Run: `git tag v1.5.0 && git push origin v1.5.0`
  - Document in setup instructions
  - **One-time manual step**

---

## Verification

After all phases complete:

1. [ ] Create test commit with `feat: test feature`
2. [ ] Verify CHANGELOG.md is updated
3. [ ] Verify version bumped to 1.6.0
4. [ ] Verify GitHub Release created
5. [ ] Create PR with manual version mismatch
6. [ ] Verify validation fails
7. [ ] Verify changelog preview appears in PR summary
