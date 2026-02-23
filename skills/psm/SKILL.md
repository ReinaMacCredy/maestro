---
name: psm
description: "Manages isolated development sessions using git worktrees and tmux. Use when running parallel tasks that require separate environments."
argument-hint: "review <ref> | fix <ref> | feature <name> | list | attach <session> | kill <session> | cleanup | status"
aliases: [session, worktree-session]
disable-model-invocation: true
---

# Project Session Manager (/psm)

Manage isolated development sessions with git worktrees and tmux.
All PSM state lives under `~/.maestro-psm/`.

## User Command

`$ARGUMENTS`

## Supported Subcommands

| Pattern | Action |
|---|---|
| `review <ref>` | Create PR review session |
| `fix <ref>` | Create issue-fix session |
| `feature <name>` | Create feature session from `main` |
| `list` | List sessions from state + tmux |
| `attach <session>` | Show attach command for a session |
| `kill <session>` | Kill tmux + remove worktree + delete state |
| `cleanup` | Auto-clean merged PR / closed issue sessions |
| `status` | Show current session context |

---

## Initialization (Always Run First)

```bash
STATE_DIR="$HOME/.maestro-psm"
WORKTREE_ROOT="$STATE_DIR/worktrees"
LOG_DIR="$STATE_DIR/logs"
SESSIONS_FILE="$STATE_DIR/sessions.json"

mkdir -p "$WORKTREE_ROOT" "$LOG_DIR"

if [[ ! -f "$SESSIONS_FILE" ]]; then
  cat > "$SESSIONS_FILE" <<'EOF'
{"version": 1, "sessions": {}}
EOF
fi
```

**Rules:**
- Use `$HOME` (expanded `~`) for filesystem operations.
- Create parent directories before writing files or creating worktrees.
- Fail fast with clear error messages when prerequisites are missing.

---

## Shared Parsing Rules

### GitHub Reference Formats

Support these forms for `review` and `fix`:

- `#123` → current repo
- `owner/repo#123` → explicit repo
- `https://github.com/owner/repo/pull/123` → PR URL
- `https://github.com/owner/repo/issues/123` → issue URL

### Parse Outcomes

Normalize parsed data into:

- `repo`: `owner/repo`
- `number`: numeric ref (`123`)
- `kind`: `pull` or `issue`

### Validation

- `review <ref>` accepts PR formats only (`#`, `owner/repo#`, or `/pull/`).
- `fix <ref>` accepts issue formats only (`#`, `owner/repo#`, or `/issues/`).
- If `#123` is used, resolve repo from current directory with `gh repo view --json nameWithOwner`.
- If parsing fails, stop and show accepted examples.

---

## Shared Session Model

Use this consistent session metadata model in `~/.maestro-psm/sessions.json`:

```json
{
  "version": 1,
  "sessions": {
    "project:pr-123": {
      "id": "project:pr-123",
      "type": "review",
      "repo": "owner/repo",
      "number": 123,
      "ref": "#123",
      "title": "Add webhook support",
      "branch": "pr-123-review",
      "base": "main",
      "worktree": "/Users/me/.maestro-psm/worktrees/project/pr-123",
      "tmux": "maestro:project:pr-123",
      "createdAt": "2026-02-09T00:00:00Z",
      "url": "https://github.com/owner/repo/pull/123"
    }
  }
}
```

Session naming:
- Session ID: `<alias>:<ref-id>`
- Tmux session: `maestro:<session-id>`
- Alias default: repo name (from `owner/repo`)

---

## Subcommand Workflows

### 1) `review <ref>`

Goal: Create a PR review workspace from the PR head branch.

1. Parse `<ref>` into `repo` + `pr_number`.
2. Fetch PR metadata:
   ```bash
   gh pr view <num> --repo <repo> --json number,title,headRefName,baseRefName,url
   ```
3. Resolve local repo root:
   - Prefer current git repo when remotes match `<repo>`.
   - Otherwise use a dedicated local clone under `~/.maestro-psm/repos/<owner>__<repo>`.
4. Create/update review branch + worktree:
   ```bash
   git -C <repo_root> fetch origin pull/<num>/head:pr-<num>-review
   git -C <repo_root> worktree add <worktree_path> pr-<num>-review
   ```
   where `worktree_path=~/.maestro-psm/worktrees/<alias>/pr-<num>`.
5. Create tmux session in background:
   ```bash
   tmux new-session -d -s maestro:<alias>:pr-<num> -c <worktree_path>
   ```
6. Update `~/.maestro-psm/sessions.json` with session metadata.
7. Print **Session Ready** report including attach command.

---

### 2) `fix <ref>`

Goal: Create issue-fix branch from `main`, plus isolated worktree + tmux.

1. Parse `<ref>` into `repo` + `issue_number`.
2. Fetch issue metadata via `gh issue view`.
3. Create branch from `main`:
   ```bash
   git -C <repo_root> fetch origin main
   git -C <repo_root> worktree add -b fix/<num>-<slug> <worktree_path> origin/main
   ```
   where `worktree_path=~/.maestro-psm/worktrees/<alias>/issue-<num>`.
4. Create tmux session:
   ```bash
   tmux new-session -d -s maestro:<alias>:issue-<num> -c <worktree_path>
   ```
5. Update sessions.json.
6. Print **Session Ready** report with attach command.

---

### 3) `feature <name>`

Goal: Create feature branch from `main` in an isolated session.

1. Resolve current repo (`gh repo view --json nameWithOwner`).
2. Slugify `<name>` for branch-safe naming.
3. Create branch + worktree:
   ```bash
   git -C <repo_root> fetch origin main
   git -C <repo_root> worktree add -b feature/<slug> <worktree_path> origin/main
   ```
   where `worktree_path=~/.maestro-psm/worktrees/<alias>/feat-<slug>`.
4. Create tmux session:
   ```bash
   tmux new-session -d -s maestro:<alias>:feat-<slug> -c <worktree_path>
   ```
5. Update sessions.json.
6. Print **Session Ready** report with attach command.

---

### 4) `list`

Goal: Show persisted sessions and whether tmux is alive.

1. Read `~/.maestro-psm/sessions.json`.
2. Get live tmux sessions:
   ```bash
   tmux list-sessions -F "#{session_name}" 2>/dev/null
   ```
3. Cross-reference each session’s `tmux` value.
4. Display table:
   - Session ID
   - Type
   - Ref
   - Branch
   - Status (`active` if tmux exists, otherwise `dead`)

---

### 5) `attach <session>`

Goal: Provide exact attach command.

1. Build tmux name: `maestro:<session>`.
2. Verify session exists:
   ```bash
   tmux has-session -t maestro:<session>
   ```
3. If found, print:
   ```bash
   tmux attach -t maestro:<session>
   ```
4. If missing, show clear error and suggest `/psm list`.

---

### 6) `kill <session>`

Goal: Fully remove one session.

1. Resolve session record from sessions.json.
2. Kill tmux session:
   ```bash
   tmux kill-session -t maestro:<session>
   ```
3. Remove worktree:
   ```bash
   git -C <repo_root> worktree remove <worktree_path> --force
   ```
4. Delete session from sessions.json.
5. Report removed tmux/worktree/state entries.

---

### 7) `cleanup`

Goal: Auto-clean completed review/fix sessions.

For each session in sessions.json:

- If `type=review`, check merge status:
  ```bash
  gh pr view <num> --repo <repo> --json merged
  ```
  If merged, remove session (same steps as `kill`).

- If `type=fix`, check close status:
  ```bash
  gh issue view <num> --repo <repo> --json closed
  ```
  If closed, remove session (same steps as `kill`).

- If `type=feature`, skip automatic cleanup unless explicitly requested.

Output summary:
- Cleaned sessions
- Skipped sessions
- Any failures with reason

---

### 8) `status`

Goal: Show session context for current environment.

Detection order:
1. If inside tmux, read current session name (`tmux display-message -p '#S'`).
2. Else match current working directory against stored worktree paths in sessions.json.
3. If a match is found, print full session details:
   - ID
   - Type
   - Repo/Ref
   - Branch
   - Worktree
   - Tmux name
   - Attach command
4. If no match, report: "Not currently in a managed PSM session."

---

## Session Report Format

Use this exact structure when creating a session:

```text
Session Ready!

  ID:       project:pr-123
  Type:     review
  PR:       #123 - Add webhook support
  Worktree: ~/.maestro-psm/worktrees/project/pr-123
  Tmux:     maestro:project:pr-123

Commands:
  Attach:  tmux attach -t maestro:project:pr-123
  Kill:    /psm kill project:pr-123
  Cleanup: /psm cleanup
```

For issue/fix or feature sessions, replace the PR line with the relevant reference label.

---

## Important Notes

- Use `gh` CLI for all GitHub operations (auth + repo context).
- Use `~/.maestro-psm/` for all state (never `~/.psm/`, never `.maestro/`).
- Expand `~` to `$HOME` in real command execution.
- Create parent directories before filesystem operations.
- Tmux sessions launch in background; user remains in current terminal.
- Always tell the user how to attach (`tmux attach -t maestro:<session>`).
- Use clear, actionable error messages (invalid ref, missing gh auth, missing tmux session, git worktree errors).
