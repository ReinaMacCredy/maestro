# Find TODOs

I'll locate all TODO comments and unfinished work markers in your codebase.

First, let me scan for various TODO patterns:

```bash
# Common TODO patterns across different languages
echo "Searching for TODOs, FIXMEs, and other markers..."

# Count different types (excluding common build/dependency directories)
TODO_COUNT=$(grep -r -E "TODO|Todo|todo" --exclude-dir=.git --exclude-dir=vendor --exclude-dir=deps --exclude-dir=dependencies --exclude-dir=build --exclude-dir=dist --exclude-dir=target --exclude-dir=out . 2>/dev/null | wc -l)
FIXME_COUNT=$(grep -r -E "FIXME|Fixme|fixme" --exclude-dir=.git --exclude-dir=vendor --exclude-dir=deps --exclude-dir=dependencies --exclude-dir=build --exclude-dir=dist --exclude-dir=target --exclude-dir=out . 2>/dev/null | wc -l)
HACK_COUNT=$(grep -r -E "HACK|Hack|hack" --exclude-dir=.git --exclude-dir=vendor --exclude-dir=deps --exclude-dir=dependencies --exclude-dir=build --exclude-dir=dist --exclude-dir=target --exclude-dir=out . 2>/dev/null | wc -l)
NOTE_COUNT=$(grep -r -E "NOTE:|Note:|XXX" --exclude-dir=.git --exclude-dir=vendor --exclude-dir=deps --exclude-dir=dependencies --exclude-dir=build --exclude-dir=dist --exclude-dir=target --exclude-dir=out . 2>/dev/null | wc -l)

echo "Found: $TODO_COUNT TODOs, $FIXME_COUNT FIXMEs, $HACK_COUNT HACKs, $NOTE_COUNT NOTEs"
```

I'll search for these patterns:
- **TODO/Todo/todo**: General tasks to complete
- **FIXME/Fixme/fixme**: Known issues that need fixing
- **HACK/Hack/hack**: Temporary workarounds
- **XXX**: Warnings or problematic code
- **NOTE/Note**: Important notes that might indicate incomplete work
- Various comment styles based on your project's languages

For each marker found, I'll show:
1. **File location** with line number
2. **The full comment** with context
3. **Surrounding code** to understand what needs to be done
4. **Priority assessment** based on the marker type

I'll organize findings by:
- **Critical** (FIXME, HACK, XXX): Issues that could cause problems
- **Important** (TODO): Features or improvements needed
- **Informational** (NOTE): Context that might need attention

I'll also identify:
- TODOs that reference missing implementations
- Placeholder code that needs replacement
- Incomplete error handling
- Stubbed functions awaiting implementation

This helps you track and prioritize unfinished work in your codebase.