#!/usr/bin/env bash
set -u
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
BIN="$ROOT/target/debug/maestro"
RUN_GATES=1

usage() {
  cat <<'USAGE'
Usage: scripts/verify-all.sh [--workflows-only]

Runs Maestro's full local verification gates, then exercises the built CLI as a
black-box through greenfield and brownfield temp-project workflows.

Options:
  --workflows-only  Skip cargo check/fmt/clippy/test and run only CLI workflows.
  -h, --help        Show this help.

Environment:
  MAESTRO_VERIFY_ROOT  Optional output root for temp projects and logs.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workflows-only)
      RUN_GATES=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "$ROOT/Cargo.toml" ]]; then
  echo "FAIL: script must live under the Maestro repo" >&2
  exit 1
fi

if [[ -n "${MAESTRO_VERIFY_ROOT:-}" ]]; then
  ARTIFACT_ROOT="$MAESTRO_VERIFY_ROOT"
  mkdir -p "$ARTIFACT_ROOT"
else
  ARTIFACT_ROOT="$(mktemp -d /private/tmp/maestro-verify-all.XXXXXX)"
fi

LOG="$ARTIFACT_ROOT/logs"
PASS_FILE="$ARTIFACT_ROOT/passes.txt"
mkdir -p "$LOG"
: > "$PASS_FILE"

fail() {
  local message="$1"
  echo "FAIL: $message" >&2
  echo "artifact root: $ARTIFACT_ROOT" >&2
  echo "logs: $LOG" >&2
  exit 1
}

pass() {
  local label="$1"
  shift
  printf '%s\n' "$label" >> "$PASS_FILE"
  printf 'PASS %-32s %s\n' "$label" "$*"
}

step() {
  printf '\n==> %s\n' "$*"
}

run_repo() {
  local label="$1"
  shift
  run_in "$label" "$ROOT" 0 "$@"
}

run_in() {
  local label="$1"
  local dir="$2"
  local expected="$3"
  shift 3
  local outfile="$LOG/$label.out"
  printf '+ %s\n' "$*" > "$outfile"
  (
    cd "$dir" || exit 125
    "$@"
  ) >> "$outfile" 2>&1
  local rc=$?
  if [[ "$expected" == "fail" ]]; then
    if [[ "$rc" -eq 0 ]]; then
      cat "$outfile" >&2
      fail "$label expected failure"
    fi
  elif [[ "$rc" -ne "$expected" ]]; then
    cat "$outfile" >&2
    fail "$label rc=$rc expected=$expected"
  fi
  pass "$label" "rc=$rc"
}

contains() {
  local label="$1"
  local needle="$2"
  grep -Fq -- "$needle" "$LOG/$label.out" || {
    cat "$LOG/$label.out" >&2
    fail "$label missing: $needle"
  }
}

not_contains() {
  local label="$1"
  local needle="$2"
  if grep -Fq -- "$needle" "$LOG/$label.out"; then
    cat "$LOG/$label.out" >&2
    fail "$label unexpectedly contained: $needle"
  fi
}

json_assert() {
  local label="$1"
  local expr="$2"
  python3 - "$LOG/$label.out" "$expr" <<'PY'
import json
import sys

path, expr = sys.argv[1], sys.argv[2]
raw = open(path, encoding="utf-8").read()
start = raw.find("{")
end = raw.rfind("}")
if start == -1 or end == -1 or end < start:
    raise SystemExit(f"no json object found in {path}\n{raw}")
data = json.loads(raw[start:end + 1])
if not eval(expr, {"data": data}):
    raise SystemExit(f"json assertion failed: {expr}\n{json.dumps(data, indent=2)}")
PY
}

assert_path_exists() {
  local path="$1"
  [[ -e "$path" ]] || fail "missing path: $path"
}

assert_path_missing() {
  local path="$1"
  [[ ! -e "$path" ]] || fail "unexpected path exists: $path"
}

git_init_project() {
  local dir="$1"
  mkdir -p "$dir"
  (
    cd "$dir" || exit 125
    git init -q
  ) || fail "git init failed for $dir"
}

write_qa_baseline() {
  local dir="$1"
  local feature="$2"
  local scenario_id="$3"
  local scenario="$4"
  mkdir -p "$dir/.maestro/features/$feature"
  {
    printf '%s\n' '---'
    printf 'amend_log_position: 0\n'
    printf '%s\n\n' '---'
    printf '### QA Baseline Contract\n\n'
    printf '%s\n' '- Scenario Matrix:'
    printf '  - [%s] %s\n' "$scenario_id" "$scenario"
  } > "$dir/.maestro/features/$feature/baseline.md"
}

write_qa_slices() {
  local dir="$1"
  local feature="$2"
  local scenario_id="$3"
  local evidence="$4"
  {
    printf '%s\n' 'slices:'
    printf '  - scenarios: ["%s"]\n' "$scenario_id"
    printf '    evidence: ["manual: %s"]\n' "$evidence"
  } > "$dir/.maestro/features/$feature/qa-slices.yaml"
}

parse_first_blocker_id() {
  local label="$1"
  local blocker
  blocker="$(grep -o 'blk-[A-Za-z0-9_-]*' "$LOG/$label.out" | head -n 1 || true)"
  [[ -n "$blocker" ]] || fail "$label did not expose a blocker id"
  printf '%s\n' "$blocker"
}

run_cargo_gates() {
  step "cargo gates"
  run_repo cargo-check cargo check --all-targets
  run_repo cargo-fmt cargo fmt -- --check
  run_repo cargo-clippy cargo clippy --all-targets -- -D warnings
  run_repo cargo-test cargo test --quiet
}

build_binary() {
  step "build local CLI"
  run_repo cargo-build cargo build
  assert_path_exists "$BIN"
}

run_greenfield_workflow() {
  step "greenfield workflow"
  local work="$ARTIFACT_ROOT/greenfield"
  git_init_project "$work"

  run_in green-version "$work" 0 "$BIN" version
  contains green-version "binary:"
  run_in green-root-help "$work" 0 "$BIN" --help
  contains green-root-help "Commands:"
  run_in green-update-help "$work" 0 "$BIN" update --help
  contains green-update-help "--check"
  run_in green-uninstall-help "$work" 0 "$BIN" uninstall --help
  contains green-uninstall-help "Remove maestro hooks"
  run_in green-hook-record-help "$work" 0 "$BIN" hook record --help
  contains green-hook-record-help "Usage: maestro hook record"

  run_in green-init-dry "$work" 0 "$BIN" init --dry-run
  contains green-init-dry "dry-run writes nothing"
  assert_path_missing "$work/.maestro"

  run_in green-init "$work" 0 "$BIN" init --yes
  contains green-init "initialized maestro"
  assert_path_exists "$work/.maestro/harness/HARNESS.md"
  assert_path_exists "$work/.maestro/skills/maestro-task/SKILL.md"

  run_in green-install-codex "$work" 0 "$BIN" install --agent codex
  run_in green-install-claude "$work" 0 "$BIN" install --agent claude
  run_in green-sync-dry "$work" 0 "$BIN" sync --dry-run
  contains green-sync-dry "dry-run"
  run_in green-doctor "$work" 0 "$BIN" doctor
  contains green-doctor "doctor:"
  run_in green-status-json "$work" 0 "$BIN" status --json
  json_assert green-status-json 'data["schema"] == "maestro.status.v1"'
  run_in green-shell-init "$work" 0 "$BIN" shell-init
  contains green-shell-init "MAESTRO_CURRENT_TASK"
  run_in green-mcp-tools "$work" 0 "$BIN" mcp tools
  contains green-mcp-tools "maestro"
  run_in green-watch-snapshot "$work" 0 "$BIN" watch snapshot

  run_in green-task-create "$work" 0 "$BIN" task create "Greenfield README proof" --check "README mentions Maestro verify-all"
  contains green-task-create "created task-001"
  run_in green-task-next-json "$work" 0 "$BIN" task next --json
  json_assert green-task-next-json 'data["next_action"]["task_id"] == "task-001"'
  run_in green-task-explore "$work" 0 "$BIN" task explore task-001
  run_in green-task-accept "$work" 0 "$BIN" task accept task-001
  run_in green-task-claim "$work" 0 "$BIN" task claim task-001
  printf '%s\n' '# Greenfield' 'README mentions Maestro verify-all.' > "$work/README.md"
  run_in green-task-update "$work" 0 "$BIN" task update task-001 --summary "updated README" --claim "README mentions Maestro verify-all"
  run_in green-task-complete "$work" 0 "$BIN" task complete task-001 --summary "updated README" --claim "README mentions Maestro verify-all" --proof "observed: README mentions Maestro verify-all"
  contains green-task-complete "verification passed for task-001"
  run_in green-task-verify "$work" 0 "$BIN" task verify task-001
  contains green-task-verify "verification passed for task-001"
  run_in green-query-proof "$work" 0 "$BIN" query proof task-001
  contains green-query-proof "proof task-001:"
  contains green-query-proof "claims: 1/1"

  run_in green-manual-create "$work" 0 "$BIN" task create "Manual event proof" --check "manual event proof observed"
  run_in green-manual-explore "$work" 0 "$BIN" task explore task-002
  run_in green-manual-accept "$work" 0 "$BIN" task accept task-002
  run_in green-manual-claim "$work" 0 "$BIN" task claim task-002
  run_in green-manual-complete-fail "$work" fail "$BIN" task complete task-002 --summary "done" --claim "manual event proof observed"
  contains green-manual-complete-fail "task remains: needs_verification"
  run_in green-event-create "$work" 0 "$BIN" event create --task-id task-002 --message "manual proof" --claim "manual event proof observed" --payload '{"ok":true}'
  run_in green-manual-verify "$work" 0 "$BIN" task verify task-002
  contains green-manual-verify "verification passed for task-002"
  run_in green-query-friction "$work" 0 "$BIN" query friction

  run_in green-decision-new "$work" 0 "$BIN" decision new "Use verify-all harness"
  contains green-decision-new "decision-001"
  run_in green-decision-show "$work" 0 "$BIN" decision show decision-001
  contains green-decision-show "Use verify-all harness"
  run_in green-query-decisions "$work" 0 "$BIN" query decisions
  contains green-query-decisions "decision-001"

  run_in green-feature-new "$work" 0 "$BIN" feature new "Greenfield Export"
  run_in green-feature-set "$work" 0 "$BIN" feature set greenfield-export --acceptance "Greenfield export ships" --area "export CLI"
  run_in green-feature-accept-block "$work" fail "$BIN" feature accept greenfield-export
  contains green-feature-accept-block "skill: qa-baseline"
  write_qa_baseline "$work" greenfield-export bl-001 "Greenfield export ships"
  run_in green-feature-accept "$work" 0 "$BIN" feature accept greenfield-export
  run_in green-feature-start "$work" 0 "$BIN" feature start greenfield-export
  run_in green-feature-task-create "$work" 0 "$BIN" task create "Wire greenfield export" --feature greenfield-export
  run_in green-feature-task-explore "$work" 0 "$BIN" task explore task-003
  run_in green-feature-task-accept "$work" 0 "$BIN" task accept task-003
  contains green-feature-task-accept "verify+ inherited from feature"
  run_in green-feature-task-claim "$work" 0 "$BIN" task claim task-003
  run_in green-feature-task-complete "$work" 0 "$BIN" task complete task-003 --summary "wired export" --claim "Greenfield export ships" --proof "observed: Greenfield export ships"
  contains green-feature-task-complete "feature ready:"
  run_in green-feature-ship-block "$work" fail "$BIN" feature ship greenfield-export --outcome "Greenfield export shipped"
  contains green-feature-ship-block "skill: qa-slice"
  write_qa_slices "$work" greenfield-export bl-001 "Greenfield export ships"
  run_in green-feature-ship-dry "$work" 0 "$BIN" feature ship greenfield-export --outcome "Greenfield export shipped" --dry-run
  contains green-feature-ship-dry "writes: none"
  run_in green-feature-ship "$work" 0 "$BIN" feature ship greenfield-export --outcome "Greenfield export shipped"
  contains green-feature-ship "ship receipt:"

  run_in green-query-matrix "$work" 0 "$BIN" query matrix
  contains green-query-matrix "greenfield-export"
  run_in green-harness-list "$work" 0 "$BIN" harness list --all
  run_in green-query-backlog "$work" 0 "$BIN" query backlog
  run_in green-task-doctor "$work" 0 "$BIN" task doctor
  run_in green-task-archive "$work" 0 "$BIN" task archive task-001
  contains green-task-archive "restore: maestro task unarchive task-001"
  run_in green-task-unarchive "$work" 0 "$BIN" task unarchive task-001
  contains green-task-unarchive "restore receipt:"
  run_in green-feature-archive "$work" 0 "$BIN" feature archive greenfield-export
  contains green-feature-archive "restore: maestro feature unarchive greenfield-export"
  run_in green-feature-unarchive "$work" 0 "$BIN" feature unarchive greenfield-export
  contains green-feature-unarchive "restore receipt:"
  run_in green-uninstall-codex "$work" 0 "$BIN" uninstall --agent codex
  run_in green-uninstall-claude "$work" 0 "$BIN" uninstall --agent claude
}

run_brownfield_workflow() {
  step "brownfield workflow"
  local work="$ARTIFACT_ROOT/brownfield"
  git_init_project "$work"

  run_in brown-init "$work" 0 "$BIN" init --yes
  run_in brown-install-codex "$work" 0 "$BIN" install --agent codex
  mkdir -p "$work/.maestro/skills/local-only"
  printf '%s\n' '# Local Only' 'User-owned local skill.' > "$work/.maestro/skills/local-only/SKILL.md"
  run_in brown-sync "$work" 0 "$BIN" sync
  assert_path_exists "$work/.maestro/skills/local-only/SKILL.md"

  run_in brown-task-create "$work" 0 "$BIN" task create "Brownfield resume task"
  run_in brown-task-block "$work" 0 "$BIN" task block task-001 --reason "waiting on fixture"
  run_in brown-task-show-blocked "$work" 0 "$BIN" task show task-001
  contains brown-task-show-blocked "waiting on fixture"
  local blocker_id
  blocker_id="$(parse_first_blocker_id brown-task-show-blocked)"
  run_in brown-current-status "$work" 0 env MAESTRO_CURRENT_TASK=task-001 "$BIN" status
  contains brown-current-status "task-001"
  run_in brown-task-unblock "$work" 0 "$BIN" task unblock task-001 --blocker "$blocker_id"
  run_in brown-task-set "$work" 0 "$BIN" task set task-001 --check "brownfield proof observed"
  run_in brown-task-explore "$work" 0 "$BIN" task explore task-001
  run_in brown-task-accept "$work" 0 "$BIN" task accept task-001
  run_in brown-task-claim "$work" 0 "$BIN" task claim task-001
  run_in brown-task-complete "$work" 0 "$BIN" task complete task-001 --summary "resumed existing project" --claim "brownfield proof observed" --proof "observed: brownfield proof observed"
  contains brown-task-complete "verification passed for task-001"

  run_in brown-reject-create "$work" 0 "$BIN" task create "Brownfield rejected task" --check "not needed"
  run_in brown-reject "$work" 0 "$BIN" task reject task-002 --reason "out of scope"
  contains brown-reject "terminal receipt:"
  run_in brown-reject-archive "$work" 0 "$BIN" task archive task-002
  run_in brown-reject-unarchive "$work" 0 "$BIN" task unarchive task-002

  run_in brown-replacement-create "$work" 0 "$BIN" task create "Brownfield replacement" --check "replacement exists"
  run_in brown-old-create "$work" 0 "$BIN" task create "Brownfield old task" --check "old task replaced"
  run_in brown-supersede "$work" 0 "$BIN" task supersede task-004 --by task-003 --reason "covered by replacement"
  contains brown-supersede "terminal receipt:"

  run_in brown-feature-new "$work" 0 "$BIN" feature new "Brownfield Cancel"
  run_in brown-feature-set "$work" 0 "$BIN" feature set brownfield-cancel --acceptance "Brownfield cancel path covered" --area "cleanup"
  write_qa_baseline "$work" brownfield-cancel bl-001 "Brownfield cancel path covered"
  run_in brown-feature-accept "$work" 0 "$BIN" feature accept brownfield-cancel
  run_in brown-feature-amend "$work" 0 "$BIN" feature amend brownfield-cancel --reason "brownfield scope grew" --add-question "Should archived tasks be restored?"
  run_in brown-feature-start "$work" 0 "$BIN" feature start brownfield-cancel
  run_in brown-feature-child "$work" 0 "$BIN" task create "Brownfield child" --feature brownfield-cancel
  run_in brown-feature-cancel-dry "$work" 0 "$BIN" feature cancel brownfield-cancel --reason "covered by existing release" --dry-run
  contains brown-feature-cancel-dry "writes: none"
  run_in brown-feature-cancel "$work" 0 "$BIN" feature cancel brownfield-cancel --reason "covered by existing release"
  contains brown-feature-cancel "cancel receipt:"
  run_in brown-feature-archive "$work" 0 "$BIN" feature archive brownfield-cancel
  run_in brown-feature-unarchive "$work" 0 "$BIN" feature unarchive brownfield-cancel

  run_in brown-init-dry-existing "$work" 0 "$BIN" init --dry-run
  contains brown-init-dry-existing "dry-run writes nothing"
  run_in brown-doctor "$work" 0 "$BIN" doctor
  run_in brown-status "$work" 0 "$BIN" status
  run_in brown-status-json "$work" 0 "$BIN" status --json
  json_assert brown-status-json 'data["schema"] == "maestro.status.v1"'
  run_in brown-task-list-all "$work" 0 "$BIN" task list --all
  contains brown-task-list-all "task-001"
  run_in brown-feature-list-all "$work" 0 "$BIN" feature list --all
  contains brown-feature-list-all "brownfield-cancel"
  run_in brown-query-matrix "$work" 0 "$BIN" query matrix
  run_in brown-query-friction "$work" 0 "$BIN" query friction
  run_in brown-mcp-list "$work" 0 "$BIN" mcp list
  contains brown-mcp-list "maestro"
  run_in brown-watch-snapshot "$work" 0 "$BIN" watch snapshot
}

main() {
  printf 'Maestro verify-all\n'
  printf 'repo: %s\n' "$ROOT"
  printf 'artifact root: %s\n' "$ARTIFACT_ROOT"
  printf 'logs: %s\n' "$LOG"

  if [[ "$RUN_GATES" -eq 1 ]]; then
    run_cargo_gates
  else
    step "cargo gates skipped"
    pass cargo-gates-skipped "--workflows-only"
  fi

  build_binary
  run_greenfield_workflow
  run_brownfield_workflow

  printf '\nALL_MAESTRO_VERIFY_PASS\n'
  printf 'passes: %s\n' "$(wc -l < "$PASS_FILE" | tr -d ' ')"
  printf 'artifact root: %s\n' "$ARTIFACT_ROOT"
  printf 'logs: %s\n' "$LOG"
}

main "$@"
