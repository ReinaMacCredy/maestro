#!/usr/bin/env bash
# Install continuity hooks globally for Claude Code
# 
# Usage: ./scripts/install-global-hooks.sh [--repo-hooks]
#
# Options:
#   --repo-hooks  Also configure current repo to use hooks
#
# This script:
# 1. Checks for Node.js/npm
# 2. Copies source files to ~/.claude/hooks/
# 3. Builds TypeScript
# 4. Merges hooks into Claude Code settings

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
HOOKS_SRC="${REPO_ROOT}/hooks/continuity"
GLOBAL_HOOKS_DIR="${HOME}/.claude/hooks"
CLAUDE_SETTINGS="${HOME}/.claude/settings.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check dependencies
check_deps() {
    if ! command -v node &> /dev/null; then
        log_error "Node.js is required but not installed."
        log_error "Install from: https://nodejs.org/"
        exit 1
    fi
    
    if ! command -v npm &> /dev/null; then
        log_error "npm is required but not installed."
        exit 1
    fi
    
    local node_version
    node_version=$(node -v | sed 's/v//' | cut -d. -f1)
    if [[ "$node_version" -lt 18 ]]; then
        log_error "Node.js 18+ required. Current: $(node -v)"
        exit 1
    fi
    
    log_info "Dependencies OK: Node.js $(node -v), npm $(npm -v)"
}

# Copy source files
copy_files() {
    log_info "Copying hooks to ${GLOBAL_HOOKS_DIR}..."
    
    mkdir -p "${GLOBAL_HOOKS_DIR}"
    
    # Copy package.json, tsconfig.json, and src/
    cp "${HOOKS_SRC}/package.json" "${GLOBAL_HOOKS_DIR}/"
    cp "${HOOKS_SRC}/tsconfig.json" "${GLOBAL_HOOKS_DIR}/"
    
    mkdir -p "${GLOBAL_HOOKS_DIR}/src"
    cp "${HOOKS_SRC}/src/continuity.ts" "${GLOBAL_HOOKS_DIR}/src/"
    
    log_info "Files copied successfully"
}

# Build TypeScript
build_hooks() {
    log_info "Building TypeScript hooks..."
    
    cd "${GLOBAL_HOOKS_DIR}"
    npm install --silent
    npm run build --silent
    
    if [[ ! -f "${GLOBAL_HOOKS_DIR}/dist/continuity.js" ]]; then
        log_error "Build failed: dist/continuity.js not created"
        exit 1
    fi
    
    log_info "Build successful: ${GLOBAL_HOOKS_DIR}/dist/continuity.js"
}

# Merge hooks into settings.json
merge_settings() {
    log_info "Merging hooks into Claude Code settings..."
    
    local hooks_config="${HOOKS_SRC}/settings-hooks.json"
    
    if [[ ! -f "$CLAUDE_SETTINGS" ]]; then
        log_info "Creating new settings.json..."
        cp "$hooks_config" "$CLAUDE_SETTINGS"
        return
    fi
    
    # Check if jq is available for proper merge
    if command -v jq &> /dev/null; then
        log_info "Using jq for settings merge..."
        
        # Create backup
        cp "$CLAUDE_SETTINGS" "${CLAUDE_SETTINGS}.bak"
        
        # Merge hooks
        local temp_file="${CLAUDE_SETTINGS}.tmp.$$"
        jq -s '
            .[0] as $existing |
            .[1].hooks as $new_hooks |
            $existing | .hooks = ((.hooks // {}) * $new_hooks)
        ' "$CLAUDE_SETTINGS" "$hooks_config" > "$temp_file"
        
        mv "$temp_file" "$CLAUDE_SETTINGS"
        log_info "Settings merged successfully"
    else
        log_warn "jq not installed. Showing manual merge instructions..."
        log_warn "Add the following to ${CLAUDE_SETTINGS}:"
        cat "$hooks_config"
        echo ""
        log_warn "Or install jq and re-run: brew install jq"
    fi
}

# Main
main() {
    echo "=== Continuity Hooks Installer ==="
    echo ""
    
    check_deps
    copy_files
    build_hooks
    merge_settings
    
    echo ""
    log_info "Installation complete!"
    log_info "Hooks installed to: ${GLOBAL_HOOKS_DIR}"
    echo ""
    echo "Verify with: node ${GLOBAL_HOOKS_DIR}/dist/continuity.js --version"
    echo ""
    
    # Handle --repo-hooks flag
    if [[ "${1:-}" == "--repo-hooks" ]]; then
        log_info "Configuring repo hooks..."
        # Future: add repo-specific hook config here
        log_warn "--repo-hooks not yet implemented"
    fi
}

main "$@"
