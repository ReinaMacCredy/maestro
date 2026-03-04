#!/usr/bin/env bash
#
# transform-ecc-skills.sh
#
# Transforms all ECC skill SKILL.md files using metadata from skill-metadata.json.
# Rewrites frontmatter, replaces "When to Activate" sections, and appends
# a Maestro workflow footer.
#
# Usage: bash scripts/transform-ecc-skills.sh
# Assumes: run from repo root; jq installed; skillpacks/ecc/ exists.

set -euo pipefail

METADATA_JSON="skillpacks/ecc/skill-metadata.json"
SKILLS_DIR="skillpacks/ecc/skills"
TRANSFORMED=0
TOTAL=0

# -------------------------------------------------------------------
# Preflight
# -------------------------------------------------------------------
if [[ ! -f "$METADATA_JSON" ]]; then
  echo "[!] Metadata file not found: $METADATA_JSON" >&2
  exit 1
fi

if [[ ! -d "$SKILLS_DIR" ]]; then
  echo "[!] Skills directory not found: $SKILLS_DIR" >&2
  exit 1
fi

# -------------------------------------------------------------------
# Helper: read a metadata field for a given skill name
#   usage: meta_field <skill-name> <jq-filter>
# -------------------------------------------------------------------
meta_field() {
  local skill_name="$1"
  local filter="$2"
  jq -r --arg name "$skill_name" '.[("maestro:" + $name)] | '"$filter" "$METADATA_JSON"
}

# -------------------------------------------------------------------
# Helper: read a metadata array as YAML inline array
#   usage: meta_yaml_array <skill-name> <field>
#   output: [item1, item2]
# -------------------------------------------------------------------
meta_yaml_array() {
  local skill_name="$1"
  local field="$2"
  jq -r --arg name "$skill_name" --arg f "$field" \
    '.[("maestro:" + $name)][$f] // [] | "[" + (map(tostring) | join(", ")) + "]"' \
    "$METADATA_JSON"
}

# -------------------------------------------------------------------
# Helper: read maestro_phases as key-value pairs
#   usage: meta_phases <skill-name>
#   output: lines of "phase_name<TAB>description"
# -------------------------------------------------------------------
meta_phases() {
  local skill_name="$1"
  jq -r --arg name "$skill_name" \
    '.[("maestro:" + $name)].maestro_phases // {} | to_entries[] | "\(.key)\t\(.value)"' \
    "$METADATA_JSON"
}

# -------------------------------------------------------------------
# Helper: read related_skills as array of strings
#   usage: meta_related <skill-name>
#   output: one skill name per line
# -------------------------------------------------------------------
meta_related() {
  local skill_name="$1"
  jq -r --arg name "$skill_name" \
    '.[("maestro:" + $name)].related // [] | .[]' \
    "$METADATA_JSON"
}

# -------------------------------------------------------------------
# Helper: read lifecycle as comma-separated string for display
#   usage: meta_lifecycle_display <skill-name>
#   output: "implement, review"
# -------------------------------------------------------------------
meta_lifecycle_display() {
  local skill_name="$1"
  jq -r --arg name "$skill_name" \
    '.[("maestro:" + $name)].lifecycle // [] | join(", ")' \
    "$METADATA_JSON"
}

# -------------------------------------------------------------------
# Transform a single SKILL.md
# -------------------------------------------------------------------
transform_skill() {
  local dir="$1"
  local skill_file="$dir/SKILL.md"

  if [[ ! -f "$skill_file" ]]; then
    echo "[!] No SKILL.md in $dir -- skipping" >&2
    return 1
  fi

  # Extract skill name from directory: maestro:foo -> foo
  local dir_basename
  dir_basename="$(basename "$dir")"
  local skill_name="${dir_basename#maestro:}"

  # Check skill exists in metadata
  local found
  found="$(jq -r --arg name "maestro:$skill_name" 'has($name) | tostring' "$METADATA_JSON")"
  if [[ "$found" != "true" ]]; then
    echo "[!] Skill '$skill_name' not found in metadata -- skipping" >&2
    return 1
  fi

  # Read metadata values
  local description
  description="$(meta_field "$skill_name" '.description')"
  # Write description to temp file (avoids -v escape interpretation issues)
  local description_tmpfile
  description_tmpfile="$(mktemp)"
  printf '%s' "$description" > "$description_tmpfile"
  local lifecycle_yaml
  lifecycle_yaml="$(meta_yaml_array "$skill_name" 'lifecycle')"
  local domain_yaml
  domain_yaml="$(meta_yaml_array "$skill_name" 'domain')"
  local lifecycle_display
  lifecycle_display="$(meta_lifecycle_display "$skill_name")"

  # Build the "Maestro Integration" replacement block
  local integration_block
  integration_block="## Maestro Integration"$'\n'$'\n'
  integration_block+="**Lifecycle**: ${lifecycle_display}"$'\n'
  integration_block+="**Activates when**: maestro:new-track detects relevant tech in tech-stack.md, or maestro:implement encounters matching task types."$'\n'$'\n'
  integration_block+="### Phase Guidance"$'\n'

  while IFS=$'\t' read -r phase_name phase_desc; do
    [[ -z "$phase_name" ]] && continue
    integration_block+="**In maestro:${phase_name}**: ${phase_desc}"$'\n'
  done < <(meta_phases "$skill_name")

  integration_block+=$'\n'"### Related Skills"$'\n'
  while IFS= read -r related; do
    [[ -z "$related" ]] && continue
    integration_block+="- ${related}"$'\n'
  done < <(meta_related "$skill_name")

  # Build the footer
  local footer_block
  footer_block=$'\n'"---"$'\n'
  footer_block+="## Relationship to Maestro Workflow"$'\n'
  footer_block+="- \`/maestro:new-track\` -- Detects this skill during Step 9.5 (skill matching)"$'\n'
  footer_block+="- \`/maestro:implement\` -- Loads this skill's guidance during task execution"$'\n'
  footer_block+="- \`/maestro:review\` -- Uses checklists as review criteria"$'\n'

  # ---------------------------------------------------------------
  # Phase 1: Transform frontmatter
  # Phase 2: Replace "When to Activate" / "When to Use" / "Trigger"
  # Phase 3: Append footer (idempotent)
  # ---------------------------------------------------------------

  local tmpfile
  tmpfile="$(mktemp)"

  # Use awk for the full transformation in a single pass
  # We escape the replacement blocks for awk by writing them to temp files
  local integration_tmpfile
  integration_tmpfile="$(mktemp)"
  printf '%s' "$integration_block" > "$integration_tmpfile"

  local footer_tmpfile
  footer_tmpfile="$(mktemp)"
  printf '%s' "$footer_block" > "$footer_tmpfile"

  awk \
    -v lifecycle_yaml="$lifecycle_yaml" \
    -v domain_yaml="$domain_yaml" \
    -v description_file="$description_tmpfile" \
    -v integration_file="$integration_tmpfile" \
    -v footer_file="$footer_tmpfile" \
    '
    BEGIN {
      in_frontmatter = 0
      frontmatter_count = 0
      in_activation_section = 0
      activation_replaced = 0
      in_footer_section = 0
      prev_was_separator = 0
      held_line = ""

      # Read description from file (preserves special chars)
      getline description < description_file
      close(description_file)
      # Escape embedded double quotes for YAML output
      gsub(/"/, "\\\"", description)

      # Read integration block from file
      integration_block = ""
      while ((getline line < integration_file) > 0) {
        integration_block = integration_block line "\n"
      }
      close(integration_file)

      # Read footer block from file
      footer_block = ""
      while ((getline line < footer_file) > 0) {
        footer_block = footer_block line "\n"
      }
      close(footer_file)
    }

    # All logic in a single block for clean control flow
    {
      # ----------------------------------------------------------
      # State: inside footer section -- skip everything
      # ----------------------------------------------------------
      if (in_footer_section) {
        next
      }

      # ----------------------------------------------------------
      # State: inside activation section -- skip until next ##
      # ----------------------------------------------------------
      if (in_activation_section) {
        # Exit on ## headers that are NOT activation-type headers
        is_activation_header = ($0 ~ /^## When to Activate/ || $0 ~ /^## When to Use/ || $0 ~ /^## Trigger/ || $0 ~ /^## Maestro Integration/)
        is_sub_header = ($0 ~ /^### Phase Guidance/ || $0 ~ /^### Related Skills/)
        if (/^## / && !is_activation_header) {
          in_activation_section = 0
          # Emit blank line separator before next section
          print ""
          # fall through to print this line
        } else {
          next
        }
      }

      # ----------------------------------------------------------
      # Frontmatter boundary: ---
      # ----------------------------------------------------------
      if ($0 ~ /^---[[:space:]]*$/) {
        frontmatter_count++
        if (frontmatter_count == 1) {
          in_frontmatter = 1
          print
          next
        }
        if (frontmatter_count == 2) {
          in_frontmatter = 0
          print "lifecycle: " lifecycle_yaml
          print "domain: " domain_yaml
          print
          next
        }
        # Third+ --- : might be footer separator, buffer it
        if (prev_was_separator) {
          print held_line
        }
        held_line = $0
        prev_was_separator = 1
        next
      }

      # ----------------------------------------------------------
      # Inside frontmatter: filter and rewrite
      # ----------------------------------------------------------
      if (in_frontmatter) {
        if ($0 ~ /^origin:/) next
        if ($0 ~ /^version:/) next
        if ($0 ~ /^tools:/) next
        if ($0 ~ /^lifecycle:/) next
        if ($0 ~ /^domain:/) next
        if ($0 ~ /^description:/) {
          print "description: \"" description "\""
          next
        }
        print
        next
      }

      # ----------------------------------------------------------
      # Detect footer header (idempotency: strip old footer)
      # ----------------------------------------------------------
      if ($0 ~ /^## Relationship to Maestro Workflow/) {
        in_footer_section = 1
        prev_was_separator = 0
        next
      }

      # ----------------------------------------------------------
      # Flush any held separator before processing body lines
      # ----------------------------------------------------------
      if (prev_was_separator) {
        print held_line
        prev_was_separator = 0
      }

      # ----------------------------------------------------------
      # Detect activation/integration section headers
      # ----------------------------------------------------------
      if ($0 ~ /^## When to Activate/ || $0 ~ /^## When to Use/ || $0 ~ /^## Trigger/ || $0 ~ /^## Maestro Integration/) {
        if (!activation_replaced) {
          in_activation_section = 1
          activation_replaced = 1
          # Flush buffered blanks as separator before integration block
          if (trailing_blanks != "") {
            printf "%s", trailing_blanks
            trailing_blanks = ""
          }
          printf "%s", integration_block
          next
        }
      }

      # ----------------------------------------------------------
      # Normal content: buffer trailing blank lines
      # ----------------------------------------------------------
      if ($0 ~ /^[[:space:]]*$/) {
        trailing_blanks = trailing_blanks "\n"
      } else {
        if (trailing_blanks != "") {
          printf "%s", trailing_blanks
          trailing_blanks = ""
        }
        print
      }
    }

    END {
      if (prev_was_separator && !in_footer_section) {
        print held_line
      }
      # trailing_blanks intentionally NOT flushed -- trim trailing whitespace
      printf "%s", footer_block
    }
    ' "$skill_file" > "$tmpfile"

  # Replace original file
  mv "$tmpfile" "$skill_file"
  rm -f "$description_tmpfile" "$integration_tmpfile" "$footer_tmpfile"

  echo "[ok] maestro:${skill_name}"
  return 0
}

# -------------------------------------------------------------------
# Main loop
# -------------------------------------------------------------------
for skill_dir in "$SKILLS_DIR"/maestro:*/; do
  # Remove trailing slash
  skill_dir="${skill_dir%/}"

  # Skip if not a directory
  [[ -d "$skill_dir" ]] || continue

  TOTAL=$((TOTAL + 1))

  if transform_skill "$skill_dir"; then
    TRANSFORMED=$((TRANSFORMED + 1))
  fi
done

echo ""
echo "Transformed ${TRANSFORMED}/${TOTAL} skills"

if [[ "$TOTAL" -eq 0 ]]; then
  echo "[!] No skill directories found in $SKILLS_DIR" >&2
  exit 1
fi
