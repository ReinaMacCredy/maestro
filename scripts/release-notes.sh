#!/usr/bin/env bash
# Build a categorized "What's Changed" changelog from the conventional-commit
# subjects (and first body line) in a git range, as GitHub-flavored markdown on
# stdout. Used by .github/workflows/release.yml; runnable locally for a preview:
#   scripts/release-notes.sh <prev-tag>..HEAD
#
# Commits are grouped by conventional type into Features / Fixes / Performance /
# Documentation / Maintenance (everything else). Each entry is the subject with
# its `type(scope): ` prefix stripped, plus the first non-empty body line
# indented beneath it -- so a squash-merged release whose detail lives in the
# commit body still renders meaningfully, not as a bare one-liner.
set -euo pipefail

range="${1:-HEAD}"

# 0x1F separates subject from body within a record; 0x1E separates records.
# Both are control bytes that cannot appear in a commit message, so a multi-line
# body never confuses the field/record split.
us=$(printf '\037')
rs=$(printf '\036')

git log "$range" --no-merges --format="%x1e%s%x1f%b" |
  awk -v RS="$rs" -v FS="$us" -v hdr="## What's Changed" '
  function trim(s) { gsub(/^[ \t\r]+|[ \t\r]+$/, "", s); return s }
  # GitHub renders the body as GFM and runs an HTML sanitizer, so unescaped
  # <id> / Vec<String> / <sha> in commit text get stripped as bogus tags. Escape
  # &<> -- ampersand FIRST, and via \& (literal-&) since bare & in a gsub
  # replacement means the matched text.
  function esc(s) {
    gsub(/&/, "\\&amp;", s)
    gsub(/</, "\\&lt;", s)
    gsub(/>/, "\\&gt;", s)
    return s
  }
  {
    subject = trim($1)
    if (subject == "") next
    if (subject ~ /^Merge /) next

    type = ""; desc = subject
    if (match(subject, /^[a-z]+(\([^)]*\))?!?: /)) {
      pfx = substr(subject, 1, RLENGTH)
      type = pfx; sub(/[(!:].*/, "", type)
      desc = substr(subject, RLENGTH + 1)
    }

    section = "Maintenance"
    if (type == "feat") section = "Features"
    else if (type == "fix") section = "Fixes"
    else if (type == "perf") section = "Performance"
    else if (type == "docs") section = "Documentation"

    first = ""
    n = split($2, lines, "\n")
    for (i = 1; i <= n; i++) {
      l = trim(lines[i])
      if (l == "") continue
      if (tolower(l) ~ /^(co-authored-by|signed-off-by|reviewed-by|acked-by|co-developed-by):/) continue
      first = l; break
    }

    entries[section] = entries[section] "* " esc(desc) "\n"
    if (first != "") entries[section] = entries[section] "  " esc(first) "\n"
    seen[section] = 1
  }
  END {
    order[1] = "Features"; order[2] = "Fixes"; order[3] = "Performance"
    order[4] = "Documentation"; order[5] = "Maintenance"
    any = 0
    for (k = 1; k <= 5; k++) if (order[k] in seen) any = 1
    if (!any) exit 0
    print hdr "\n"
    for (k = 1; k <= 5; k++) {
      s = order[k]
      if (s in seen) printf "### %s\n%s\n", s, entries[s]
    }
  }
  '
