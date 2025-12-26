#!/bin/bash
# Validate markdown links - find broken references to files
# Usage: ./scripts/validate-links.sh [directory]

set -euo pipefail

DIR="${1:-.}"
ERRORS=0

echo "Checking markdown links in $DIR..."

# Find all markdown files and check their links
while IFS= read -r file; do
    # Extract markdown links: [text](path)
    while IFS= read -r link; do
        # Skip URLs (http/https), anchors-only (#), and empty
        if [[ "$link" =~ ^https?:// ]] || [[ "$link" =~ ^# ]] || [[ -z "$link" ]]; then
            continue
        fi
        
        # Remove anchor from path
        path="${link%%#*}"
        
        # Skip if empty after anchor removal
        [[ -z "$path" ]] && continue
        
        # Resolve relative path from file's directory
        base_dir=$(dirname "$file")
        resolved="$base_dir/$path"
        
        # Normalize path
        resolved=$(cd "$base_dir" 2>/dev/null && realpath -m "$path" 2>/dev/null || echo "$resolved")
        
        if [[ ! -e "$resolved" ]]; then
            echo "BROKEN: $file -> $link"
            ((ERRORS++)) || true
        fi
    done < <(grep -oE '\]\([^)]+\)' "$file" 2>/dev/null | sed 's/\](\(.*\))/\1/' | grep -v '^$' || true)
done < <(find "$DIR" -name "*.md" -type f 2>/dev/null)

if [[ $ERRORS -eq 0 ]]; then
    echo "✓ All links valid"
    exit 0
else
    echo "✗ Found $ERRORS broken links"
    exit 1
fi
