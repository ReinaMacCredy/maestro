#!/bin/bash
# beads-metrics-summary.sh
# 
# Aggregates usage metrics from .conductor/metrics.jsonl
# and prints a weekly summary.
#
# Usage: ./scripts/beads-metrics-summary.sh [--days N]
#
# Options:
#   --days N    Number of days to include (default: 7)

set -e

METRICS_FILE=".conductor/metrics.jsonl"
DAYS="${1:-7}"

# Parse --days flag
if [[ "$1" == "--days" ]]; then
  DAYS="${2:-7}"
fi

# Check if metrics file exists
if [[ ! -f "$METRICS_FILE" ]]; then
  echo "No metrics file found at $METRICS_FILE"
  echo "Metrics are logged when Conductor commands run with beads integration."
  exit 0
fi

# Calculate cutoff date
if [[ "$(uname)" == "Darwin" ]]; then
  CUTOFF=$(date -v-${DAYS}d +%Y-%m-%dT00:00:00Z)
else
  CUTOFF=$(date -d "${DAYS} days ago" +%Y-%m-%dT00:00:00Z)
fi

echo "═══════════════════════════════════════════════════════════════"
echo "  BEADS METRICS SUMMARY (last ${DAYS} days)"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Count total events
TOTAL=$(wc -l < "$METRICS_FILE" | tr -d ' ')
echo "Total events (all time): $TOTAL"
echo ""

# Filter events within date range and aggregate
echo "Events in period:"
echo "─────────────────────────────────────────────────────────────────"

# Count by event type
jq -r --arg cutoff "$CUTOFF" '
  select(.timestamp >= $cutoff) | .event
' "$METRICS_FILE" 2>/dev/null | sort | uniq -c | sort -rn | while read count event; do
  printf "  %-30s %5d\n" "$event" "$count"
done

echo ""
echo "─────────────────────────────────────────────────────────────────"

# MA attempts
MA_ATTEMPTS=$(jq -r --arg cutoff "$CUTOFF" '
  select(.timestamp >= $cutoff and .event == "ma_attempt") | .mode
' "$METRICS_FILE" 2>/dev/null | wc -l | tr -d ' ')

if [[ "$MA_ATTEMPTS" -gt 0 ]]; then
  MA_SUCCESS=$(jq -r --arg cutoff "$CUTOFF" '
    select(.timestamp >= $cutoff and .event == "ma_attempt" and .mode == "MA") | .mode
  ' "$METRICS_FILE" 2>/dev/null | wc -l | tr -d ' ')
  
  echo ""
  echo "Multi-Agent Mode:"
  echo "  Attempts: $MA_ATTEMPTS"
  echo "  Successful MA: $MA_SUCCESS"
  echo "  Fallback to SA: $((MA_ATTEMPTS - MA_SUCCESS))"
fi

# TDD cycles
TDD_CYCLES=$(jq -r --arg cutoff "$CUTOFF" '
  select(.timestamp >= $cutoff and .event == "tdd_cycle") | .phase
' "$METRICS_FILE" 2>/dev/null | wc -l | tr -d ' ')

if [[ "$TDD_CYCLES" -gt 0 ]]; then
  echo ""
  echo "TDD Cycles:"
  
  # Count by phase
  jq -r --arg cutoff "$CUTOFF" '
    select(.timestamp >= $cutoff and .event == "tdd_cycle") | .phase
  ' "$METRICS_FILE" 2>/dev/null | sort | uniq -c | while read count phase; do
    printf "  %-15s %5d\n" "$phase" "$count"
  done
  
  # Average duration per phase
  echo ""
  echo "  Avg duration (seconds):"
  for phase in RED GREEN REFACTOR; do
    AVG=$(jq -r --arg cutoff "$CUTOFF" --arg phase "$phase" '
      select(.timestamp >= $cutoff and .event == "tdd_cycle" and .phase == $phase) | .duration
    ' "$METRICS_FILE" 2>/dev/null | awk '{s+=$1; c++} END {if(c>0) printf "%.0f", s/c; else print "0"}')
    if [[ "$AVG" != "0" ]]; then
      printf "    %-12s %5s\n" "$phase" "${AVG}s"
    fi
  done
fi

# Manual bd usage
MANUAL_BD=$(jq -r --arg cutoff "$CUTOFF" '
  select(.timestamp >= $cutoff and .event == "manual_bd") | .command
' "$METRICS_FILE" 2>/dev/null | wc -l | tr -d ' ')

if [[ "$MANUAL_BD" -gt 0 ]]; then
  echo ""
  echo "Manual bd Commands (outside Conductor):"
  jq -r --arg cutoff "$CUTOFF" '
    select(.timestamp >= $cutoff and .event == "manual_bd") | .command
  ' "$METRICS_FILE" 2>/dev/null | sort | uniq -c | sort -rn | head -5 | while read count cmd; do
    printf "  %-30s %5d\n" "$cmd" "$count"
  done
fi

# Handoff events
HANDOFFS=$(jq -r --arg cutoff "$CUTOFF" '
  select(.timestamp >= $cutoff and .event == "handoff_expired") | .file
' "$METRICS_FILE" 2>/dev/null | wc -l | tr -d ' ')

if [[ "$HANDOFFS" -gt 0 ]]; then
  echo ""
  echo "⚠️  Expired Handoffs: $HANDOFFS"
  echo "   Consider checking agent availability"
fi

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "═══════════════════════════════════════════════════════════════"
