# Doc Scanner

Logic for scanning markdown files and extracting code references.

## Scan Algorithm

### Step 1: Find Markdown Files

```bash
# Find all .md files, excluding common non-project directories
find . -name "*.md" \
  -not -path "./node_modules/*" \
  -not -path "./.git/*" \
  -not -path "./vendor/*" \
  -not -path "./.beads/*" \
  -not -path "./conductor/archive/*"
```

**Default scan paths:**
- Project root (`README.md`, `CONTRIBUTING.md`, etc.)
- `docs/` directory
- Any `.md` files in `src/` or `lib/` (inline documentation)

### Step 2: Extract Code References

For each markdown file, scan for:

1. **Explicit file paths** - References to source files
2. **Import statements** - In code blocks
3. **Function/class names** - In backticks or code blocks
4. **Relative links** - Markdown links to code files

### Step 3: Build Dependency Map

Create a map of `doc → [referenced code files]`:

```json
{
  "docs": [
    {
      "path": "README.md",
      "references": [
        {"type": "file_path", "value": "src/index.ts", "line": 15},
        {"type": "function", "value": "initApp", "line": 42}
      ]
    },
    {
      "path": "docs/api.md",
      "references": [
        {"type": "file_path", "value": "src/api/routes.ts", "line": 8},
        {"type": "import", "value": "src/utils.ts", "line": 23}
      ]
    }
  ]
}
```

## Code Reference Patterns

### 1. File Path References

Detect explicit file paths in text:

```regex
# Matches paths like src/file.ts, ./lib/utils.js, etc.
(?:^|[\s\`\(\[])
  (
    (?:\.{0,2}/)?           # Optional ./ or ../
    (?:src|lib|app|pkg|cmd|internal|api|components|pages|routes|services|utils|hooks|types|models|controllers|middleware)/
    [\w\-./]+               # Path segments
    \.(?:ts|tsx|js|jsx|py|go|rs|java|rb|php|swift|kt|scala|c|cpp|h|hpp|cs|vue|svelte)
  )
(?:[\s\`\)\]]|$)
```

**Examples matched:**
- `src/index.ts`
- `./lib/utils.js`
- `components/Button.tsx`
- `api/routes/users.py`

### 2. Import Statements in Code Blocks

Detect imports within fenced code blocks:

```regex
# JavaScript/TypeScript imports
import\s+.*\s+from\s+['"]([^'"]+)['"]
require\s*\(\s*['"]([^'"]+)['"]\s*\)

# Python imports
from\s+([\w.]+)\s+import
import\s+([\w.]+)

# Go imports
import\s+(?:\([\s\S]*?\)|"([^"]+)")
```

**Examples matched:**
```typescript
import { foo } from './utils'     // → ./utils
import * as bar from '../lib/bar' // → ../lib/bar
const x = require('./config')     // → ./config
```

### 3. Function/Class Names in Backticks

Detect code references in inline backticks:

```regex
# Single backtick code spans
`([A-Z][a-zA-Z0-9]*(?:\.[a-zA-Z0-9]+)?)`      # Classes: `MyClass`, `Utils.helper`
`([a-z][a-zA-Z0-9]*\([^)]*\))`                 # Functions: `doThing()`, `init(config)`
`([a-z][a-zA-Z0-9]*)`                          # Variables/functions: `config`, `handleClick`
```

**Examples matched:**
- `` `MyClass` `` → class reference
- `` `initApp()` `` → function call
- `` `config` `` → variable/constant

### 4. Markdown Links to Code Files

Detect links pointing to source files:

```regex
\[([^\]]+)\]\(([^)]+\.(?:ts|tsx|js|jsx|py|go|rs|java|rb|php))\)
```

**Examples matched:**

```text
[source](src/index.ts) → src/index.ts
[utils](./lib/utils.js#L42) → ./lib/utils.js
```

### 5. Code Blocks with File Comments

Detect file references in code block comments:

```regex
# File path in comment at start of code block
```(?:typescript|javascript|python|go)
\s*(?://|#)\s*(?:file:|File:)?\s*(.+\.\w+)
```

**Examples matched:**
```typescript
// file: src/config.ts
export const config = {...}
```

## Output Format

### Dependency Map JSON

```json
{
  "scanned_at": "2025-12-27T10:00:00Z",
  "project_root": "/path/to/project",
  "docs": [
    {
      "path": "README.md",
      "last_modified": "2025-12-26T15:30:00Z",
      "references": [
        {
          "type": "file_path",
          "value": "src/index.ts",
          "line": 15,
          "column": 10,
          "context": "See `src/index.ts` for the main entry point."
        },
        {
          "type": "function",
          "value": "initApp",
          "line": 42,
          "column": 5,
          "context": "Call `initApp()` to initialize."
        }
      ]
    }
  ],
  "summary": {
    "total_docs": 5,
    "total_references": 23,
    "unique_code_files": 12
  }
}
```

### Reference Types

| Type | Description | Example |
|------|-------------|---------|
| `file_path` | Explicit path to source file | `src/utils.ts` |
| `import` | Import/require statement | `from './lib'` |
| `function` | Function call or reference | `initApp()` |
| `class` | Class reference | `MyComponent` |
| `link` | Markdown link to code | `[link](file.ts)` |

## Performance Considerations

### Caching

Cache scan results in `.doc-sync-cache.json`:

```json
{
  "last_scan": "2025-12-27T10:00:00Z",
  "file_hashes": {
    "README.md": "abc123",
    "docs/api.md": "def456"
  },
  "dependency_map": {...}
}
```

On subsequent scans, only re-scan files with changed hashes.

### Limits

- Max files to scan: 500
- Max file size: 1MB
- Max references per file: 100

---

*See [SKILL.md](../../SKILL.md) for full workflow.*
