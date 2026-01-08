# CI/CD Version Migration: 5.x.x → 0.5.x

## 1. Problem Statement

Migrate version scheme from 5.x.x to 0.5.x format. This affects:
- All version generation and bumping logic
- Changelog formatting
- Git tags
- Plugin manifest files

## 2. Discovery (Phase 1)

### Architecture Snapshot

| Component | File | Purpose |
|-----------|------|---------|
| Version source | `.claude-plugin/plugin.json` | Current: `"5.0.0"` |
| Version sync | `.claude-plugin/marketplace.json` | Synced by CI |
| Release workflow | `.github/workflows/release.yml` | Bump + tag + changelog |
| Validation workflow | `.github/workflows/validate.yml` | PR checks |
| Changelog config | `cliff.toml` | git-cliff template |
| Labels | `.github/labels.yml` | `release:major/minor/patch/skip` |

### Existing Patterns

**Version calculation** ([release.yml#L138-L162](file:///Users/maccredyreina/Documents/Projects/_Active/my-workflow%3A3/.github/workflows/release.yml#L138-L162)):
- Parse current version from plugin.json
- Determine bump type from PR labels or commit prefixes
- Increment appropriate segment

**Tag pattern** (cliff.toml L53):
```toml
tag_pattern = "v[0-9].*"
```

### Technical Constraints

- git-cliff tag pattern must match new format
- Existing tags (v5.0.0, v4.x.x) remain in history
- CHANGELOG.md has entries under old format

## 3. Gap Analysis + Risk Map

### Gap Analysis

| Area | Current | Target | Gap |
|------|---------|--------|-----|
| Version source | `5.0.0` in plugin.json | `0.5.0` | Manual reset required |
| Bump semantics | `major` → MAJOR+1 | Stay in 0.x | `release:major` would jump to 1.0.0 |
| Tag pattern | `v[0-9].*` | Works for 0.x | No change needed |
| Changelog | 5.x.x at top | 0.5.x at top | Visual regression (confusing) |
| External consumers | Expect monotonic versions | 0.5.0 after 5.0.0 | Possible rejection/confusion |

### Risk Map

| Component | Risk | Reason |
|-----------|------|--------|
| `release.yml` version calc | **MEDIUM** | `release:major` jumps to 1.0.0 |
| Tag/tag discovery | **LOW** | Pattern already supports 0.x |
| git-cliff / changelog | **LOW-MEDIUM** | Works but visual regression |
| CHANGELOG.md content | **LOW** | Static, just needs note |
| plugin.json / marketplace.json | **MEDIUM** | Downgrade might confuse consumers |
| External consumers | **MEDIUM-HIGH** | Monotonic version expectations |
| Developer workflow | **LOW-MEDIUM** | Conceptual confusion |

## 4. Approach Options

### Option A: One-time Reset (Minimal Changes)
- Manually set plugin.json to `0.5.0`
- Tag current main as `v0.5.0`
- Add "Versioning reset" note to CHANGELOG
- **Pros**: Minimal CI changes
- **Cons**: `release:major` exits 0.x immediately

### Option B: Reset + Pre-1.0 Bump Rules
- Same reset as A, **plus** modify release.yml:
  - While MAJOR=0: `release:major` → increment MINOR (0.5→0.6)
  - Only intentional action moves to 1.0.0
- **Pros**: Aligns with pre-1.0 semver interpretation
- **Cons**: Requires release.yml changes + testing

### Option C: New Lineage / Namespace
- Treat 0.5.x as separate release line
- Adjust cliff.toml tag_pattern to `v0.*`
- Possibly separate channel/package ID
- **Pros**: Clean separation from 5.x history
- **Cons**: Higher complexity

## 5. Spike Results

No HIGH risk items requiring spikes. All risks are MEDIUM or below.

## 6. Final Approach: Option C - Clean Break

### Changes Required

1. **Reset plugin.json + marketplace.json** → `0.5.0`
2. **Update cliff.toml** → tag_pattern `v0.*` to ignore 1-5.x history
3. **Archive old CHANGELOG.md** → move to CHANGELOG-legacy.md
4. **Create fresh CHANGELOG.md** → starts at 0.5.0
5. **Update release.yml** → pre-1.0 bump semantics (major→minor while 0.x)
6. **Create initial tag** → `v0.5.0`
7. **Update docs** → AGENTS.md versioning section
