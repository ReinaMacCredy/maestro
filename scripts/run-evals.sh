#!/usr/bin/env bash
# run-evals.sh -- Run maestro skill evaluations
#
# Usage:
#   ./scripts/run-evals.sh trigger           # Run trigger evals (all 9 skills)
#   ./scripts/run-evals.sh trigger setup     # Run trigger evals for one skill
#   ./scripts/run-evals.sh functional        # Run functional evals (all skills)
#   ./scripts/run-evals.sh functional 1 3 6  # Run specific eval IDs
#   ./scripts/run-evals.sh all               # Run both trigger + functional
#
# Options:
#   --model <id>        Model to use (default: claude-sonnet-4-6)
#   --workers <n>       Parallel workers for trigger evals (default: 5)
#   --runs <n>          Runs per trigger query (default: 3)
#   --no-baseline       Skip baseline runs in functional evals
#   --open              Open eval viewer after functional evals finish
#   --help              Show this help

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKILL_CREATOR="${HOME}/.claude/skills/skill-creator"
EVALS_DIR="${REPO_ROOT}/evals"
WORKSPACE="${REPO_ROOT}/maestro-workspace"

# Defaults
MODEL="claude-sonnet-4-6"
WORKERS=5
RUNS_PER_QUERY=3
SKIP_BASELINE=false
OPEN_VIEWER=false

# ── Helpers ──────────────────────────────────────────────────────────────────

die()  { echo "[!] $*" >&2; exit 1; }
info() { echo "--> $*" >&2; }
ok()   { echo "[ok] $*" >&2; }

usage() {
    head -20 "$0" | grep '^#' | sed 's/^# \?//'
    exit 0
}

# ── Skill map (bash 3.2 compatible -- no associative arrays) ─────────────────

SKILL_KEYS="setup AGENTS.md new-track design implement review status note revert"

skill_path() {
    case "$1" in
        setup)     echo "${REPO_ROOT}/skills/maestro:setup" ;;
        AGENTS.md) echo "${REPO_ROOT}/skills/maestro:AGENTS.md" ;;
        new-track) echo "${REPO_ROOT}/skills/maestro:new-track" ;;
        design)    echo "${REPO_ROOT}/skills/maestro:design" ;;
        implement) echo "${REPO_ROOT}/skills/maestro:implement" ;;
        review)    echo "${REPO_ROOT}/skills/maestro:review" ;;
        status)    echo "${REPO_ROOT}/skills/maestro:status" ;;
        note)      echo "${REPO_ROOT}/skills/maestro:note" ;;
        revert)    echo "${REPO_ROOT}/skills/maestro:revert" ;;
        *)         echo "" ;;
    esac
}

# ── Parse args ───────────────────────────────────────────────────────────────

MODE=""
SKILL_FILTER=""
EVAL_IDS=""

while [ $# -gt 0 ]; do
    case "$1" in
        trigger|functional|all)
            MODE="$1"; shift ;;
        --model)
            MODEL="$2"; shift 2 ;;
        --workers)
            WORKERS="$2"; shift 2 ;;
        --runs)
            RUNS_PER_QUERY="$2"; shift 2 ;;
        --no-baseline)
            SKIP_BASELINE=true; shift ;;
        --open)
            OPEN_VIEWER=true; shift ;;
        --help|-h)
            usage ;;
        *)
            if [ "$MODE" = "trigger" ] && [ -z "$SKILL_FILTER" ]; then
                SKILL_FILTER="$1"; shift
            elif [ "$MODE" = "functional" ]; then
                EVAL_IDS="${EVAL_IDS} $1"; shift
            else
                die "Unknown argument: $1"
            fi
            ;;
    esac
done

[ -n "$MODE" ] || usage

# ── Trigger evals ────────────────────────────────────────────────────────────

run_trigger_evals() {
    local trigger_file="${EVALS_DIR}/trigger-evals.json"
    [ -f "$trigger_file" ] || die "Missing ${trigger_file}"

    local results_dir="${WORKSPACE}/trigger-results"
    mkdir -p "$results_dir"

    # Build list of skills to test
    local keys_to_test=""
    if [ -n "$SKILL_FILTER" ]; then
        for key in $SKILL_KEYS; do
            case "$key" in
                *"$SKILL_FILTER"*) keys_to_test="${keys_to_test} ${key}" ;;
            esac
        done
        keys_to_test=$(echo "$keys_to_test" | xargs)
        [ -n "$keys_to_test" ] || die "No skill matching filter: $SKILL_FILTER"
    else
        keys_to_test="$SKILL_KEYS"
    fi

    local skill_count
    skill_count=$(echo "$keys_to_test" | wc -w | xargs)
    info "Running trigger evals for ${skill_count} skill(s) | model=${MODEL} workers=${WORKERS} runs=${RUNS_PER_QUERY}"

    for key in $keys_to_test; do
        local sp
        sp=$(skill_path "$key")
        local skill_eval_file="${results_dir}/${key}-queries.json"
        local skill_result_file="${results_dir}/${key}-results.json"

        info "Preparing eval set for maestro:${key}..."

        # Extract queries relevant to this skill:
        # - should_trigger == "maestro:<key>" -> true
        # - everything else -> false
        jq --arg skill "maestro:${key}" '[
            .evals[] |
            if .should_trigger == $skill then
                {query: .query, should_trigger: true}
            elif .should_trigger == false then
                {query: .query, should_trigger: false}
            else
                {query: .query, should_trigger: false}
            end
        ]' "$trigger_file" > "$skill_eval_file"

        local query_count
        query_count=$(jq 'length' "$skill_eval_file")
        info "  maestro:${key}: ${query_count} queries"

        # Run eval using skill-creator's run_eval.py
        PYTHONPATH="${SKILL_CREATOR}" python3 -m scripts.run_eval \
            --eval-set "$skill_eval_file" \
            --skill-path "$sp" \
            --model "$MODEL" \
            --num-workers "$WORKERS" \
            --runs-per-query "$RUNS_PER_QUERY" \
            --timeout 30 \
            --verbose \
            > "$skill_result_file" 2>&1 || true

        if [ -f "$skill_result_file" ] && jq -e '.summary' "$skill_result_file" > /dev/null 2>&1; then
            local passed failed total
            passed=$(jq '.summary.passed' "$skill_result_file")
            total=$(jq '.summary.total' "$skill_result_file")
            failed=$(jq '.summary.failed' "$skill_result_file")
            ok "maestro:${key}: ${passed}/${total} passed (${failed} failed)"
        else
            echo "[!] maestro:${key}: eval run failed or produced invalid output" >&2
        fi
    done

    # Summary
    echo ""
    info "=== Trigger Eval Summary ==="
    for key in $keys_to_test; do
        local result_file="${results_dir}/${key}-results.json"
        if [ -f "$result_file" ] && jq -e '.summary' "$result_file" > /dev/null 2>&1; then
            local passed total
            passed=$(jq '.summary.passed' "$result_file")
            total=$(jq '.summary.total' "$result_file")
            printf "  %-15s %s/%s\n" "maestro:${key}" "$passed" "$total"
        else
            printf "  %-15s %s\n" "maestro:${key}" "ERROR"
        fi
    done
    echo ""
    info "Full results: ${results_dir}/"
}

# ── Functional evals ─────────────────────────────────────────────────────────

next_iteration() {
    local i=1
    while [ -d "${WORKSPACE}/iteration-${i}" ]; do
        i=$((i + 1))
    done
    echo "$i"
}

run_functional_evals() {
    local evals_file="${EVALS_DIR}/evals.json"
    [ -f "$evals_file" ] || die "Missing ${evals_file}"

    local iteration
    iteration=$(next_iteration)
    local iter_dir="${WORKSPACE}/iteration-${iteration}"
    mkdir -p "$iter_dir"

    info "Functional evals: iteration ${iteration} | model=${MODEL}"

    # Determine which evals to run
    local eval_ids_json
    EVAL_IDS=$(echo "$EVAL_IDS" | xargs)
    if [ -n "$EVAL_IDS" ]; then
        eval_ids_json=$(echo "$EVAL_IDS" | tr ' ' '\n' | jq -R 'tonumber' | jq -s '.')
        info "Running eval IDs: ${EVAL_IDS}"
    else
        eval_ids_json=$(jq '[.evals[].id]' "$evals_file")
        info "Running all $(jq '.evals | length' "$evals_file") evals"
    fi

    # Remove CLAUDECODE env var for nesting
    unset CLAUDECODE 2>/dev/null || true

    # Process each eval
    echo "$eval_ids_json" | jq -r '.[]' | while read -r eval_id; do
        local eval_json
        eval_json=$(jq --argjson id "$eval_id" '.evals[] | select(.id == $id)' "$evals_file")

        if [ -z "$eval_json" ]; then
            echo "[!] No eval found with id=${eval_id}, skipping" >&2
            continue
        fi

        local skill prompt eval_name
        skill=$(echo "$eval_json" | jq -r '.skill')
        prompt=$(echo "$eval_json" | jq -r '.prompt')
        eval_name=$(echo "$eval_json" | jq -r '.skill' | sed 's/maestro://')-eval-${eval_id}
        local eval_dir="${iter_dir}/${eval_name}"

        info "Eval ${eval_id}: ${eval_name}"

        # ── With-skill run ──
        local with_dir="${eval_dir}/with_skill"
        mkdir -p "${with_dir}/outputs"

        local skill_short sp
        skill_short=$(echo "$skill" | sed 's/maestro://')
        sp=$(skill_path "$skill_short")

        if [ -z "$sp" ] || [ ! -d "$sp" ]; then
            echo "[!] Skill path not found for ${skill}, skipping" >&2
            continue
        fi

        info "  [with_skill] Running..."
        local start_ms end_ms duration_ms
        start_ms=$(python3 -c 'import time; print(int(time.time()*1000))')

        local claude_output
        claude_output=$(claude -p "$prompt" \
            --model "$MODEL" \
            --output-format json \
            --allowedTools "Read,Write,Edit,Bash,Glob,Grep,Skill" \
            2>/dev/null) || true

        end_ms=$(python3 -c 'import time; print(int(time.time()*1000))')
        duration_ms=$((end_ms - start_ms))

        # Save output
        echo "$claude_output" > "${with_dir}/raw_output.json"

        # Extract result text
        if echo "$claude_output" | jq -e '.result' > /dev/null 2>&1; then
            echo "$claude_output" | jq -r '.result' > "${with_dir}/outputs/result.txt"
        else
            echo "$claude_output" > "${with_dir}/outputs/result.txt"
        fi

        # Extract token count
        local total_tokens=0
        if echo "$claude_output" | jq -e '.usage' > /dev/null 2>&1; then
            total_tokens=$(echo "$claude_output" | jq '[.usage.input_tokens, .usage.output_tokens] | add // 0')
        fi

        # Save timing
        jq -n \
            --argjson tokens "$total_tokens" \
            --argjson dur "$duration_ms" \
            '{total_tokens: $tokens, duration_ms: $dur, total_duration_seconds: ($dur / 1000)}' \
            > "${with_dir}/timing.json"

        ok "  [with_skill] Done (${duration_ms}ms, ${total_tokens} tokens)"

        # ── Baseline run (without skill) ──
        if [ "$SKIP_BASELINE" = "false" ]; then
            local without_dir="${eval_dir}/without_skill"
            mkdir -p "${without_dir}/outputs"

            info "  [without_skill] Running..."
            start_ms=$(python3 -c 'import time; print(int(time.time()*1000))')

            claude_output=$(claude -p "$prompt" \
                --model "$MODEL" \
                --output-format json \
                --allowedTools "Read,Write,Edit,Bash,Glob,Grep" \
                2>/dev/null) || true

            end_ms=$(python3 -c 'import time; print(int(time.time()*1000))')
            duration_ms=$((end_ms - start_ms))

            echo "$claude_output" > "${without_dir}/raw_output.json"
            if echo "$claude_output" | jq -e '.result' > /dev/null 2>&1; then
                echo "$claude_output" | jq -r '.result' > "${without_dir}/outputs/result.txt"
            else
                echo "$claude_output" > "${without_dir}/outputs/result.txt"
            fi

            total_tokens=0
            if echo "$claude_output" | jq -e '.usage' > /dev/null 2>&1; then
                total_tokens=$(echo "$claude_output" | jq '[.usage.input_tokens, .usage.output_tokens] | add // 0')
            fi

            jq -n \
                --argjson tokens "$total_tokens" \
                --argjson dur "$duration_ms" \
                '{total_tokens: $tokens, duration_ms: $dur, total_duration_seconds: ($dur / 1000)}' \
                > "${without_dir}/timing.json"

            ok "  [without_skill] Done (${duration_ms}ms, ${total_tokens} tokens)"
        fi

        # Save eval metadata
        echo "$eval_json" | jq '{
            eval_id: .id,
            eval_name: (.skill | sub("maestro:"; "") | . + "-eval-" + (.id | tostring)),
            prompt: .prompt,
            assertions: (.assertions // [])
        }' > "${eval_dir}/eval_metadata.json"
    done

    ok "All functional evals complete: ${iter_dir}/"

    # ── Aggregate + viewer ──
    info "Aggregating benchmark..."
    PYTHONPATH="${SKILL_CREATOR}" python3 -m scripts.aggregate_benchmark \
        "$iter_dir" \
        --skill-name "maestro" 2>/dev/null || info "Aggregation skipped (script may need results in expected format)"

    if [ "$OPEN_VIEWER" = "true" ]; then
        info "Launching eval viewer..."
        local benchmark_flag=""
        if [ -f "${iter_dir}/benchmark.json" ]; then
            benchmark_flag="--benchmark ${iter_dir}/benchmark.json"
        fi
        local prev_flag=""
        if [ "$iteration" -gt 1 ] && [ -d "${WORKSPACE}/iteration-$((iteration - 1))" ]; then
            prev_flag="--previous-workspace ${WORKSPACE}/iteration-$((iteration - 1))"
        fi
        nohup python3 "${SKILL_CREATOR}/eval-viewer/generate_review.py" \
            "$iter_dir" --skill-name "maestro" \
            $benchmark_flag $prev_flag \
            > /dev/null 2>&1 &
        echo $! > "${iter_dir}/.viewer_pid"
        ok "Viewer launched (PID $(cat "${iter_dir}/.viewer_pid"))"
    fi

    echo ""
    info "=== Functional Eval Summary ==="
    info "Iteration: ${iteration}"
    info "Results:   ${iter_dir}/"
    if [ "$OPEN_VIEWER" = "true" ]; then
        info "Viewer:    running (kill with: kill $(cat "${iter_dir}/.viewer_pid" 2>/dev/null || echo '?'))"
    else
        info "To view:   ./scripts/run-evals.sh functional --open"
    fi
}

# ── Main ─────────────────────────────────────────────────────────────────────

case "$MODE" in
    trigger)
        run_trigger_evals
        ;;
    functional)
        run_functional_evals
        ;;
    all)
        run_trigger_evals
        echo ""
        run_functional_evals
        ;;
esac
