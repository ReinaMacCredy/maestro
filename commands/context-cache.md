# Cache Project Context

I'll analyze and remember key information about this project to speed up future commands.

Let me examine your project structure and save what I discover:

```bash
# Create cache directory if needed
CACHE_DIR="$HOME/.claude/.ccplugins_cache"
mkdir -p "$CACHE_DIR"

# Generate cache file name based on project path
PROJECT_HASH=$(pwd | (md5sum 2>/dev/null || md5) | cut -d' ' -f1)
CACHE_FILE="$CACHE_DIR/project_${PROJECT_HASH}.cache"
```

I'll detect and remember:
- What build tools you use
- How to run tests
- What formatter is configured
- Project structure patterns

This helps other commands run faster by not re-analyzing the same information repeatedly.

The cache expires after 24 hours or when key project files change.