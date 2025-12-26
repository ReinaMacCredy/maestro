#!/bin/bash
# Validate anchor links - verify #anchor references resolve
# Usage: ./scripts/validate-anchors.sh [directory]

set -euo pipefail

DIR="${1:-.}"
ERRORS=0

echo "Checking anchor links in $DIR..."

# Find all markdown files
while IFS= read -r file; do
    # Extract links with anchors: [text](path#anchor) or [text](#anchor)
    while IFS= read -r link; do
        # Must contain #
        [[ ! "$link" =~ \# ]] && continue
        
        # Skip external URLs
        [[ "$link" =~ ^https?:// ]] && continue
        
        # Split path and anchor
        path="${link%%#*}"
        anchor="${link#*#}"
        
        # Skip empty anchors
        [[ -z "$anchor" ]] && continue
        
        # Determine target file
        if [[ -z "$path" ]]; then
            target="$file"
        else
            base_dir=$(dirname "$file")
            target=$(cd "$base_dir" 2>/dev/null && realpath -m "$path" 2>/dev/null || echo "$base_dir/$path")
        fi
        
        # Skip if target doesn't exist (validate-links.sh handles that)
        [[ ! -f "$target" ]] && continue
        
        # Generate expected heading ID (GitHub-style: lowercase, spaces to hyphens, remove special chars)
        # Check if anchor exists as a heading in target file
        # Headings become anchors: ## My Heading -> #my-heading
        
        if ! grep -qiE "^#+[[:space:]]+.*$(echo "$anchor" | sed 's/-/ /g')" "$target" 2>/dev/null; then
            # Also check for exact anchor match in raw form
            headings=$(grep -iE "^#+[[:space:]]+" "$target" 2>/dev/null || true)
            if ! echo "$headings" | grep -qi "${anchor//-/ }"; then
                # Relaxed check: just see if the anchor text appears in any heading
                anchor_pattern=$(echo "$anchor" | sed 's/-/.*/g')
                if ! grep -qiE "^#+[[:space:]]+.*$anchor_pattern" "$target" 2>/dev/null; then
                    echo "BROKEN ANCHOR: $file -> $link"
                    ((ERRORS++)) || true
                fi
            fi
        fi
    done < <(grep -oE '\]\([^)]+\)' "$file" 2>/dev/null | sed 's/\](\(.*\))/\1/' | grep '#' || true)
done < <(find "$DIR" -name "*.md" -type f 2>/dev/null)

if [[ $ERRORS -eq 0 ]]; then
    echo "All anchors valid"
    exit 0
else
    echo "Found $ERRORS broken anchors"
    exit 1
fi
