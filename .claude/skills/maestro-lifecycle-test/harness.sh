#!/usr/bin/env bash
# maestro-lifecycle-test harness.
# Validates a built maestro binary end to end in an isolated throwaway repo and
# emits an NDJSON report (one finding per line) + a one-line human summary.
#
# Inputs (env):
#   MAESTRO_BIN  path to the prebuilt binary       (default: $REPO/target/debug/maestro)
#   MAESTRO_SRC  maestro source checkout (has embedded/)  (default: git toplevel of this script)
#   KEEP=1       keep the throwaway target repo for inspection
#
# Exit code: 0 if zero BLOCKING failures, 1 otherwise. Non-blocking fails and
# skips never fail the run (the swarm loop only fixes blocking product_bugs).
#
# Assertions are grounded in real captured output (see SPEC-test-scenarios.md §5),
# not the spec's illustrative text. maestro emits unicode em dash, arrow, ellipsis.
set -u

# ---- resolve source checkout + binary ------------------------------------
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=${MAESTRO_SRC:-$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || echo "$SCRIPT_DIR")}
BIN=${MAESTRO_BIN:-$REPO/target/debug/maestro}
EMBEDDED_SKILLS="$REPO/embedded/skills"
EMBEDDED_HOOKS="$REPO/embedded/hooks"

TOTAL=0 PASS=0 FAIL=0 SKIP=0 BFAIL=0
PHASE="setup"

# ---- preflight (test_or_env failures abort early, exit 2) ----------------
preflight_fail() { printf '{"step":"PRE","status":"fail","severity":"blocking","category":"test_or_env","title":"%s"}\n' "$1"; echo "maestro-lifecycle-test: preflight failed: $1" >&2; exit 2; }
command -v jq >/dev/null 2>&1 || preflight_fail "jq not found (required for the NDJSON report)"
command -v git >/dev/null 2>&1 || preflight_fail "git not found"
[ -x "$BIN" ] || preflight_fail "maestro binary not executable at $BIN (main must 'cargo build' first; set MAESTRO_BIN)"
[ -d "$EMBEDDED_SKILLS" ] || preflight_fail "embedded/skills not found under $REPO (set MAESTRO_SRC)"

# stale-binary guard: if any embedded/ file is newer than the binary, drift findings are
# about a binary that predates the source, so categorize them test_or_env (rebuild), not product_bug.
STALE=0
[ -n "$(find "$REPO/embedded" -type f -newer "$BIN" -print -quit 2>/dev/null)" ] && STALE=1

# ---- isolated throwaway target repo --------------------------------------
TARGET=$(mktemp -d "${TMPDIR:-/tmp}/maestro-lifecycle-test.XXXXXX")
REPORT="$TARGET/maestro-lifecycle-test.report.json"
SCRATCH="$TARGET/.scratch"; mkdir -p "$SCRATCH"
: > "$REPORT"
cd "$TARGET" || preflight_fail "cannot cd into target $TARGET"
git init -q && git config user.email t@maestro.test && git config user.name maestro-test

cleanup() { [ "${KEEP:-0}" = 1 ] || { cd /; rm -rf "$TARGET"; }; }
trap cleanup EXIT

# ---- finding + assertion helpers -----------------------------------------
# record STEP TITLE STATUS SEVERITY CATEGORY EXPECTED ACTUAL EVIDENCE FIX_AREA
record() {
  jq -nc \
    --arg step "$1" --arg phase "$PHASE" --arg title "$2" --arg status "$3" \
    --arg severity "$4" --arg category "$5" --arg expected "$6" --arg actual "$7" \
    --arg evidence "$8" --arg fix_area "$9" \
    '{step:$step,phase:$phase,title:$title,status:$status,severity:$severity,category:$category,expected:$expected,actual:$actual,evidence:$evidence,fix_area:$fix_area}' \
    | tee -a "$REPORT"
  TOTAL=$((TOTAL+1))
  case "$3" in
    pass) PASS=$((PASS+1));;
    fail) FAIL=$((FAIL+1)); [ "$4" = blocking ] && BFAIL=$((BFAIL+1));;
    skip) SKIP=$((SKIP+1));;
  esac
}

RC=0 OUT="" ERR="" LAST_CMD=""
# run STEP-context: execute maestro, capture exit/stdout/stderr into RC/OUT/ERR (+ argv into LAST_CMD)
run() { LAST_CMD="maestro $*"; "$BIN" "$@" >"$SCRATCH/o" 2>"$SCRATCH/e"; RC=$?; OUT=$(cat "$SCRATCH/o"); ERR=$(cat "$SCRATCH/e"); }

# ev: compact proof of the maestro invocation behind a finding (the command + exit + output snippet)
ev() {
  local o e
  o=$(printf '%s' "$OUT" | tr '\n' ' ' | cut -c1-120)
  e=$(printf '%s' "$ERR" | tr '\n' ' ' | cut -c1-120)
  printf '$ %s | rc=%s | out: %s | err: %s' "${LAST_CMD:-<no maestro command yet>}" "$RC" "$o" "$e"
}

# pass/fail STEP TITLE [SEVERITY] [FIX_AREA] [EVIDENCE]   (evidence defaults to ev(): the last maestro call)
pass() { record "$1" "$2" pass "${3:-blocking}" "" "" "" "${5:-$(ev)}" "${4:-}"; }
fail() { record "$1" "$2" fail "${3:-blocking}" product_bug "${6:-}" "${7:-}" "${5:-$(ev)}" "${4:-}"; }
skip() { record "$1" "$2" skip "${3:-non_blocking}" "${4:-not_built}" "" "" "${5:-}" ""; }

# assert_rc STEP TITLE WANT [FIX_AREA] [SEVERITY]   (reads $RC/$ERR/$OUT; always blocking unless overridden)
# NOTE: arg order here is FIX_AREA-then-SEVERITY -- matches its call sites, which omit severity.
#       This is the REVERSE of assert_has/file/dir below (severity-then-fix_area); intentional,
#       do not "align" them. The old order silently dropped the fix_area into severity and left
#       blocking failures uncounted (BFAIL never incremented -> gate exited green on real failures).
assert_rc() {
  if [ "$RC" = "$3" ]; then pass "$1" "$2" "${5:-blocking}" "${4:-}"
  else fail "$1" "$2" "${5:-blocking}" "${4:-}" "" "exit=$3" "exit=$RC"; fi
}
# assert_has STEP TITLE NEEDLE [SEVERITY] [FIX_AREA] [HAYSTACK(default $ERR$OUT)]
assert_has() {
  local hay="${7:-$ERR
$OUT}"
  if printf '%s' "$hay" | grep -qF -- "$3"; then pass "$1" "$2" "${4:-blocking}" "${5:-}"
  else fail "$1" "$2" "${4:-blocking}" "${5:-}" "got: $(printf '%s' "$hay" | head -c 200)" "contains: $3" "<missing>"; fi
}
# assert_file STEP TITLE PATH [SEVERITY] [FIX_AREA]
assert_file() {
  if [ -f "$3" ] && [ -s "$3" ]; then pass "$1" "$2" "${4:-blocking}" "${5:-}"
  else fail "$1" "$2" "${4:-blocking}" "${5:-}" "missing/empty: $3" "present+non-empty: $3" "<missing>"; fi
}
# assert_dir STEP TITLE PATH [SEVERITY] [FIX_AREA]
assert_dir() {
  if [ -d "$3" ]; then pass "$1" "$2" "${4:-blocking}" "${5:-}"
  else fail "$1" "$2" "${4:-blocking}" "${5:-}" "missing dir: $3" "dir: $3" "<missing>"; fi
}

# write a minimal non-empty baseline with the given [bl-NNN] ids (none = QA-C surface)
write_baseline() { # FEATURE AMENDPOS [bl-id...]
  local f="$1" pos="$2"; shift 2
  local body=""; for id in "$@"; do body="${body}[$id] scenario $id; expected: observable.\n"; done
  printf -- "---\namend_log_position: %s\n---\n# Baseline\n\n%b" "$pos" "$body" \
    > ".maestro/features/$f/baseline.md"
}
write_slice() { # FEATURE [bl-id...]
  local f="$1"; shift
  { echo "slices:"; for id in "$@"; do printf '  - scenarios: ["%s"]\n    evidence: ["replayed %s manually"]\n' "$id" "$id"; done; } \
    > ".maestro/features/$f/qa-slices.yaml"
}
# drop matching proof so `task verify` passes for the given task id + claim
drop_proof() { # TASKID CLAIM
  local d; for d in .maestro/tasks/"$1"-*; do break; done
  [ -d "$d" ] || return 1
  mkdir -p "$d/proof"; printf 'claim: %s\n\nproof artifact for %s\n' "$2" "$1" > "$d/proof/proof.md"
}

# ---------------------------------------------------------------------------
# SELFTEST (negative control): prove the BLOCKING gate actually counts a failure.
# A green run cannot distinguish a working gate from a broken one -- BFAIL stays 0
# either way when nothing fails. So SELFTEST=1 runs ONE real maestro command, asserts
# a deliberately-wrong exit, and verifies the failing BLOCKING check is counted
# (blocking_fail>=1) and the harness exits non-zero. This is the only check that
# discriminates "gate fixed" from "gate broken".
# ---------------------------------------------------------------------------
if [ "${SELFTEST:-0}" = 1 ]; then
  PHASE="selftest (negative control)"
  run init --yes
  run feature new "gate probe"   # real maestro invocation; exits 0
  assert_rc ST.1 "negative control: assert 'feature new' exits 99 (it exits 0 -> MUST fail + count blocking)" 99 "harness/gate"
  ok=no; [ "$BFAIL" -ge 1 ] && ok=yes
  jq -nc --argjson bfail "$BFAIL" --arg ok "$ok" \
    '{selftest:{blocking_fail:$bfail,gate_counts_blocking_failure:$ok}}' | tee -a "$REPORT"
  echo "maestro-lifecycle-test SELFTEST: gate_counts_blocking_failure=$ok (blocking_fail=$BFAIL)" >&2
  [ "$ok" = yes ]; exit
fi

# ===========================================================================
# P0 -- install / init completeness (request 1, DEEP)
# ===========================================================================
PHASE="P0 install+init"
[ "$STALE" = 1 ] && record P0.0 "binary is older than embedded/ -- drift findings reclassified as env" \
  skip non_blocking test_or_env "binary built from current source" "binary predates embedded/" \
  "rebuild: cargo build" "src/embedded"
run init --yes
assert_rc  P0.1 "init --yes exits 0" 0 "src/operations/init"
if [ -z "$OUT$ERR" ]; then pass P0.2 "init is silent" non_blocking
else fail P0.2 "init is silent" non_blocking "src/operations/init" "init printed output" "silent" "$OUT$ERR"; fi
for d in harness features decisions skills hooks; do assert_dir "P0.3.$d" "init scaffolds .maestro/$d" ".maestro/$d" blocking "src/operations/init"; done
for f in harness/harness.yml harness/backlog.yaml harness/HARNESS.md hooks/record.sh; do assert_file "P0.4.${f//\//_}" "init writes .maestro/$f" ".maestro/$f" blocking "src/operations/init"; done
for s in maestro-design maestro-setup maestro-task maestro-verify qa-baseline qa-slice; do assert_dir "P0.5.$s" "init creates skill dir $s" ".maestro/skills/$s" blocking "src/operations/init"; done

run install --agent claude
assert_rc  P0.6 "install --agent claude exits 0" 0 "src/operations/install"
assert_has P0.7 "install prints managed-block diff to stdout" "maestro:start" non_blocking "src/operations/install" "$OUT"
for s in maestro-design maestro-setup maestro-task maestro-verify qa-baseline qa-slice; do
  assert_file "P0.8.$s" "install populates $s/SKILL.md" ".maestro/skills/$s/SKILL.md" blocking "src/operations/install"
  for k in name version description; do
    if grep -q "^$k:" ".maestro/skills/$s/SKILL.md"; then pass "P0.9.$s.$k" "$s frontmatter has $k" non_blocking "src/embedded"
    else fail "P0.9.$s.$k" "$s frontmatter has $k" non_blocking "src/embedded" "no ^$k:" "frontmatter key $k" "<missing>"; fi
  done
  if diff -q "$EMBEDDED_SKILLS/$s/SKILL.md" ".maestro/skills/$s/SKILL.md" >/dev/null 2>&1; then pass "P0.10.$s" "installed $s == embedded (no drift)" blocking "src/operations/install"
  elif [ "$STALE" = 1 ]; then record "P0.10.$s" "installed $s == embedded (no drift)" fail non_blocking test_or_env "identical to embedded" "differs (binary predates embedded/ -- rebuild)" "stale binary" "src/embedded"
  else fail "P0.10.$s" "installed $s == embedded (no drift)" blocking "src/operations/install" "diff against embedded" "identical" "differs"; fi
done

# hooks: record.sh present+readable+non-empty in target (NOT +x, invoked via sh); events.yaml is source-only
assert_file P0.11 "target hooks/record.sh present+non-empty" ".maestro/hooks/record.sh" blocking "src/operations/install"
if [ ! -f ".maestro/hooks/events.yaml" ]; then pass P0.12 "events.yaml is NOT extracted to target (source-only)" non_blocking "src/operations/install"
else fail P0.12 "events.yaml is NOT extracted to target" non_blocking "src/operations/install" "found in target" "absent in target" "present"; fi
assert_file P0.13 "events.yaml exists in SOURCE embedded/hooks" "$EMBEDDED_HOOKS/events.yaml" non_blocking "src/embedded"

# hook WIRING: install must register record.sh against the agent's events in settings.local.json,
# and land the managed instruction blocks. "verify all (incl hooks)" = the wiring landed, not just the script.
assert_file P0.20 "install writes .claude/settings.local.json" ".claude/settings.local.json" blocking "src/operations/install"
SETTINGS=$(cat .claude/settings.local.json 2>/dev/null || true)
assert_has P0.21 "settings.local.json wires record.sh"        "record.sh"   blocking "src/operations/install" "$SETTINGS"
assert_has P0.22 "settings.local.json registers a hook event" "PostToolUse" blocking "src/operations/install" "$SETTINGS"
assert_has P0.23 "install lands the CLAUDE.md managed block"   "<!-- maestro" non_blocking "src/operations/install" "$(cat CLAUDE.md 2>/dev/null || true)"
assert_has P0.24 "install lands the AGENTS.md managed block"   "<!-- maestro" non_blocking "src/operations/install" "$(cat AGENTS.md 2>/dev/null || true)"
assert_has P0.25 "install lands the .gitignore managed block"  ">>> maestro"  non_blocking "src/operations/install" "$(cat .gitignore 2>/dev/null || true)"

run doctor
assert_rc  P0.14 "doctor exits 0" 0 "src/interfaces/cli (doctor)"
assert_has P0.15 "doctor prints 'doctor: ok'" "doctor: ok" blocking "src/interfaces/cli (doctor)" "$OUT"

# idempotency: bare re-init ERRORS (needs --merge/--force); --merge is the clean path; re-install is a no-op
# (bare `init`, not `--yes`: init-ux redefined `--yes` to mean idempotent-merge, which exits 0)
run init
assert_rc  P0.16 "bare re-init errors (conflict guard)" 1 "src/operations/init"
assert_has P0.17 "re-init names the --merge/--force recovery" "--merge" non_blocking "src/operations/init" "$ERR"
run init --merge
assert_rc  P0.18 "init --merge is idempotent (exit 0)" 0 "src/operations/init"
run install --agent claude
assert_rc  P0.19 "re-install is a clean no-op (exit 0)" 0 "src/operations/install"

# ===========================================================================
# P1 -- feature -> task chain -> QA happy path (request 2 trunk)
# ===========================================================================
PHASE="P1 feature->task->QA happy"
run feature new "CSV export"
assert_rc  P1.1 "feature new exits 0" 0 "src/interfaces/cli/feature"
assert_has P1.2 "feature new reports created (proposed)" "created feature csv-export (proposed)" blocking "src/interfaces/cli/feature" "$OUT"

run feature set csv-export --acceptance "User can export a report to CSV"
assert_has P1.3 "feature set echoes contract counts" "acceptance=1" blocking "src/interfaces/cli/feature" "$OUT"
run feature set csv-export --area "src/export"

# accept gate: still blocked on missing baseline, names all gaps + runnable fixes
run feature accept csv-export
assert_rc  P1.4 "accept blocks while contract incomplete" 1 "src/domain/feature/registry"
assert_has P1.5 "accept gate names qa-baseline gap + fix" "qa-baseline" blocking "src/domain/feature/registry" "$ERR"
assert_has P1.6 "accept gate fix line is runnable" "fix:" non_blocking "src/domain/feature/registry" "$ERR"
# dry-run: previews on stdout, exit 0, no transition
run feature accept csv-export --dry-run
assert_rc  P1.7 "accept --dry-run exits 0" 0 "src/interfaces/cli/feature"
assert_has P1.8 "accept --dry-run previews on stdout" "would block accept" blocking "src/interfaces/cli/feature" "$OUT"

# baseline present -> accept freezes the contract
write_baseline csv-export 0 bl-001
run feature accept csv-export
assert_rc  P1.9 "accept succeeds with baseline" 0 "src/domain/feature/registry"
assert_has P1.10 "accept freezes contract (-> ready)" "contract frozen" blocking "src/domain/feature/registry" "$OUT"

# frozen: set is refused, amend grows it
run feature set csv-export --acceptance "second criterion"
assert_rc  P1.11 "set after freeze is refused" 1 "src/domain/feature/registry"
assert_has P1.12 "freeze error points to amend" "contract frozen at accept" blocking "src/domain/feature/registry" "$ERR"

run feature start csv-export
assert_has P1.13 "start moves feature to in_progress" "in_progress" blocking "src/interfaces/cli/feature" "$OUT"

run task create "Implement CSV writer" --feature csv-export
assert_has P1.14 "task create returns an id" "created task-" blocking "src/interfaces/cli/task" "$OUT"

# ship blocked: live child (+ uncovered scenario); gate AGGREGATES reasons
run feature ship csv-export
assert_rc  P1.15 "ship blocks on a live child" 1 "src/domain/feature/registry"
assert_has P1.16 "ship names the live child + fix" "live child task" blocking "src/domain/feature/registry" "$ERR"
assert_has P1.17 "ship also reports uncovered scenario (aggregated)" "coverage incomplete" blocking "src/domain/feature/qa" "$ERR"

# drive the child to verified
run task claim task-001
assert_has P1.18 "claim moves task draft->in_progress" "in_progress" blocking "src/domain/task" "$OUT"
run task complete task-001 --summary "wrote csv writer" --claim "cargo test export passes"
assert_has P1.19 "complete -> needs_verification" "needs_verification" blocking "src/domain/task" "$OUT"
drop_proof task-001 "cargo test export passes"
run task verify task-001
assert_rc  P1.20 "verify passes with claim+proof" 0 "src/domain/proof"
assert_has P1.21 "verify reports proof source" "verification passed" blocking "src/domain/proof" "$OUT"

# fresh baseline + counting slice -> ship succeeds
write_baseline csv-export 0 bl-001
write_slice csv-export bl-001
run feature ship csv-export
assert_rc  P1.22 "ship succeeds (child verified + slice covers bl)" 0 "src/domain/feature/registry"
assert_has P1.23 "ship reports shipped" "shipped csv-export" blocking "src/domain/feature/registry" "$OUT"
run feature ship csv-export
assert_has P1.24 "re-ship is an idempotent no-op" "already shipped" blocking "src/domain/feature/registry" "$OUT"

# ===========================================================================
# P2 -- standalone task path
# ===========================================================================
PHASE="P2 standalone task"
run task create "Tidy the docs"
assert_has P2.1 "standalone task create returns an id" "created task-" blocking "src/interfaces/cli/task" "$OUT"
ST=$(printf '%s' "$OUT" | sed -n 's/^created \(task-[0-9]*\).*/\1/p')
run task claim "$ST"
assert_rc  P2.2 "standalone zero-check guard fires at claim" 1 "src/domain/task"
assert_has P2.3 "guard names the --check fix" "has no checks" blocking "src/domain/task" "$ERR"
run task set "$ST" --check "docs render without warnings"
run task claim "$ST"
assert_has P2.4 "standalone with a check can claim" "in_progress" blocking "src/domain/task" "$OUT"
run task complete "$ST" --summary "tidied" --claim "docs build clean"
assert_has P2.5 "standalone complete -> needs_verification" "needs_verification" blocking "src/domain/task" "$OUT"
drop_proof "$ST" "docs build clean"
run task verify "$ST"
assert_rc  P2.6 "standalone verify passes with a check + proof" 0 "src/domain/proof"

# ===========================================================================
# P3 -- edge cases (advisor-expanded; all spec-grounded)
# ===========================================================================
PHASE="P3 edge cases"

# B1 -- C5 negative: a verified child stays verified through a behavioral amend
run feature new "Report builder"
run feature set report-builder --acceptance "renders a report"
run feature set report-builder --area "src/report"
write_baseline report-builder 0 bl-001
run feature accept report-builder; run feature start report-builder
run task create "Build renderer" --feature report-builder
RB=$(printf '%s' "$OUT" | sed -n 's/^created \(task-[0-9]*\).*/\1/p')
run task claim "$RB"; run task complete "$RB" --summary "renderer built" --claim "renderer works"
drop_proof "$RB" "renderer works"; run task verify "$RB"
run task show "$RB"; before=$(printf '%s' "$OUT" | sed -n 's/^state: //p')
run feature amend report-builder --add-acceptance "also renders CSV" --reason "scope grew"
run task show "$RB"; after=$(printf '%s' "$OUT" | sed -n 's/^state: //p')
if [ "$before" = verified ] && [ "$after" = verified ]; then pass P3.1 "C5: verified child survives a behavioral amend" blocking "src/domain/feature (amend/freshness)"
else fail P3.1 "C5: verified child survives a behavioral amend" blocking "src/domain/feature (amend/freshness)" "before=$before after=$after" "verified->verified" "$before->$after"; fi

# B2 -- cancel cascade: live child abandoned, verified child stays verified
run feature new "Cascade demo"
run feature set cascade-demo --acceptance a; run feature set cascade-demo --area src/x
write_baseline cascade-demo 0 bl-001
run feature accept cascade-demo; run feature start cascade-demo
run task create "live one" --feature cascade-demo; LIVE=$(printf '%s' "$OUT" | sed -n 's/^created \(task-[0-9]*\).*/\1/p')
run task create "done one" --feature cascade-demo; DONE=$(printf '%s' "$OUT" | sed -n 's/^created \(task-[0-9]*\).*/\1/p')
run task claim "$LIVE"
run task claim "$DONE"; run task complete "$DONE" --summary s --claim "did it"; drop_proof "$DONE" "did it"; run task verify "$DONE"
run feature cancel cascade-demo --reason "pivot"
assert_has P3.2 "cancel cascade abandons the live child" "abandoned" blocking "src/domain/feature (cancel)" "$OUT"
run task show "$LIVE"; lstate=$(printf '%s' "$OUT" | sed -n 's/^state: //p')
run task show "$DONE"; dstate=$(printf '%s' "$OUT" | sed -n 's/^state: //p')
if [ "$lstate" = abandoned ] && [ "$dstate" = verified ]; then pass P3.3 "cancel: live->abandoned, verified stays" blocking "src/domain/feature (cancel)"
else fail P3.3 "cancel: live->abandoned, verified stays" blocking "src/domain/feature (cancel)" "live=$lstate done=$dstate" "abandoned/verified" "$lstate/$dstate"; fi

# D3 -- illegal-source cells: amend on Proposed, start on terminal, task accept from draft
run feature new "Proposed thing"
run feature amend proposed-thing --add-acceptance x --reason y
assert_rc  P3.4 "amend on a Proposed feature is refused" 1 "src/domain/feature/registry"
assert_has P3.5 "amend-on-proposed error points to set+accept" "not accepted" blocking "src/domain/feature/registry" "$ERR"
run feature cancel proposed-thing --reason "drop"
run feature start proposed-thing
assert_has P3.6 "start on a terminal feature is refused" "terminal" blocking "src/domain/feature/registry" "$ERR"
run task create "draft only" --feature report-builder; DR=$(printf '%s' "$OUT" | sed -n 's/^created \(task-[0-9]*\).*/\1/p')
run task accept "$DR"
assert_rc  P3.7 "task accept from draft is refused" 1 "src/domain/task"

# D4 -- cancel without --reason is a clap misuse (exit 2)
run feature new "Reasonless"
run feature cancel reasonless
assert_rc  P3.8 "cancel without --reason -> clap exit 2" 2 "src/interfaces/cli/feature"
run feature cancel reasonless --reason "ok"
run feature cancel reasonless --reason "again"
assert_has P3.9 "re-cancel is an idempotent no-op" "already cancelled" blocking "src/domain/feature/registry" "$OUT"

# D5 -- empty slice (no evidence) does not count toward coverage
run feature new "Count rule"
run feature set count-rule --acceptance a; run feature set count-rule --area src/c
write_baseline count-rule 0 bl-001
run feature accept count-rule; run feature start count-rule
printf 'slices:\n  - scenarios: ["bl-001"]\n' > .maestro/features/count-rule/qa-slices.yaml   # NO evidence
run feature ship count-rule
assert_has P3.10 "evidence-less slice does not satisfy coverage" "coverage incomplete" blocking "src/domain/feature/qa" "$ERR"

# D6 -- QA-C: zero-[bl] baseline ships with no tasks and no slices
run feature new "Docs only"
run feature set docs-only --acceptance "documented in README"; run feature set docs-only --area docs
write_baseline docs-only 0          # zero [bl-NNN] = no behavioral surface
run feature accept docs-only; run feature start docs-only
run feature ship docs-only
assert_rc  P3.11 "zero-bl baseline ships with no slices (QA-C)" 0 "src/domain/feature/qa"

# D7 -- task archive / unarchive round-trip (built; verified child of shipped csv-export)
run task archive task-001 --dry-run
assert_has P3.12 "task archive --dry-run previews" "would archive" non_blocking "src/domain/task (archive)" "$OUT"
run task archive task-001
assert_rc  P3.13 "task archive moves the task out of live scan" 0 "src/domain/task (archive)"
assert_dir P3.14 "archived task lands under .maestro/archive/tasks" ".maestro/archive/tasks" blocking "src/domain/task (archive)"
run task unarchive task-001
assert_rc  P3.15 "task unarchive restores it" 0 "src/domain/task (archive)"

# ===========================================================================
# P4 -- UX / agent-experience
# ===========================================================================
PHASE="P4 UX"

# Exit-code discipline: distinct codes for no-op (0), gate-fail (1), clap-misuse (2)
run feature ship csv-export; rc_noop=$RC
run feature new "Ux probe"; run feature set ux-probe --acceptance a; run feature set ux-probe --area src/u; write_baseline ux-probe 0 bl-001; run feature accept ux-probe; run feature start ux-probe
run feature ship ux-probe; rc_gate=$RC
run feature cancel ux-probe; rc_clap=$RC
if [ "$rc_noop" = 0 ] && [ "$rc_gate" = 1 ] && [ "$rc_clap" = 2 ]; then pass P4.1 "exit codes distinct: no-op=0, gate=1, misuse=2" blocking "src/interfaces/cli"
else fail P4.1 "exit codes distinct: no-op=0, gate=1, misuse=2" blocking "src/interfaces/cli" "noop=$rc_noop gate=$rc_gate clap=$rc_clap" "0/1/2" "$rc_noop/$rc_gate/$rc_clap"; fi

# --dry-run writes nothing: snapshot the feature dir before/after
snap_before=$(find .maestro/features/ux-probe -type f -exec sha256sum {} + 2>/dev/null | sort)
run feature ship ux-probe --dry-run
snap_after=$(find .maestro/features/ux-probe -type f -exec sha256sum {} + 2>/dev/null | sort)
if [ "$snap_before" = "$snap_after" ]; then pass P4.2 "ship --dry-run mutates nothing" blocking "src/interfaces/cli/feature"
else fail P4.2 "ship --dry-run mutates nothing" blocking "src/interfaces/cli/feature" "feature dir changed" "no delta" "files changed"; fi

# stderr/stdout separation: a blocked gate writes to stderr, leaves stdout empty
run feature ship ux-probe
if [ -n "$ERR" ] && [ -z "$OUT" ]; then pass P4.3 "gate errors go to stderr, stdout stays clean" blocking "src/interfaces/cli"
else fail P4.3 "gate errors go to stderr, stdout stays clean" blocking "src/interfaces/cli" "err=$([ -n "$ERR" ] && echo set) out=$([ -n "$OUT" ] && echo set)" "stderr set, stdout empty" "out polluted"; fi

# Actionable error: every blocked gate carries a runnable `fix:` directive (some point to a
# `maestro ...` command, some to a skill -- the universal contract is the fix: line itself).
assert_has P4.4 "blocked gate prints a fix: directive" "fix:" blocking "src/domain/feature" "$ERR"

# Self-recovery (keystone): follow the printed fix line, assert it unblocks.
# ux-probe ships once its single live child is verified and bl-001 has a counting slice.
run task create "do ux work" --feature ux-probe; UX=$(printf '%s' "$OUT" | sed -n 's/^created \(task-[0-9]*\).*/\1/p')
run task claim "$UX"; run task complete "$UX" --summary s --claim "ux done"; drop_proof "$UX" "ux done"; run task verify "$UX"
write_slice ux-probe bl-001
run feature ship ux-probe
assert_rc  P4.5 "following the printed fixes unblocks ship (no dead-end)" 0 "src/domain/feature"

# ===========================================================================
# Appendix -- SPEC-FUTURE (not built; skip, category=not_built)
# ===========================================================================
PHASE="appendix spec-future"
skip APX.1 "feature new --from-task (promote, Q-II-5) not built" non_blocking not_built "feature new takes <title> only"
skip APX.2 "feature archive/unarchive not built (task archive IS)" non_blocking not_built "no feature archive subcommand"
skip APX.3 "feature show --archived not built" non_blocking not_built "L2-L6 archive verb layer pending"

# ---- summary -------------------------------------------------------------
jq -nc --argjson total "$TOTAL" --argjson pass "$PASS" --argjson fail "$FAIL" \
   --argjson skip "$SKIP" --argjson blocking_fail "$BFAIL" \
   --arg binary "$BIN" --arg target "$TARGET" \
   '{summary:{total:$total,pass:$pass,fail:$fail,skip:$skip,blocking_fail:$blocking_fail,binary:$binary,target:$target}}' \
   | tee -a "$REPORT"

echo "maestro-lifecycle-test: $PASS pass / $FAIL fail ($BFAIL blocking) / $SKIP skip -- report: $REPORT" >&2
[ "$BFAIL" -eq 0 ]
