# Parallel Grouping Algorithm

Groups tasks by file overlap to determine which can execute in parallel.

## Core Principle

Tasks touching the **same files** must be sequential. Tasks touching **different files** can be parallel.

## Grouping Algorithm

### Pseudocode

```python
def group_by_file_scope(beads: list[Bead]) -> list[Track]:
    """
    Groups beads into parallel tracks based on file overlap.
    
    Returns list of Track objects, each containing non-overlapping beads.
    """
    tracks = []
    
    for bead in beads:
        # Extract file paths from bead (title, description, or explicit files field)
        files = extract_files(bead)
        
        # Find compatible track (no file overlap)
        placed = False
        for track in tracks:
            if not has_overlap(track.files, files):
                track.add(bead)
                track.files.update(files)
                placed = True
                break
        
        # No compatible track found - create new one
        if not placed:
            tracks.append(Track(beads=[bead], files=set(files)))
    
    return tracks


def has_overlap(set_a: set[str], set_b: set[str]) -> bool:
    """
    Check for file-level or directory-level overlap.
    """
    # Exact file match
    if set_a & set_b:
        return True
    
    # Directory-level check (fallback)
    dirs_a = {get_directory(f) for f in set_a}
    dirs_b = {get_directory(f) for f in set_b}
    
    # Only use directory overlap if file paths are unclear/missing
    if not set_a or not set_b:
        return bool(dirs_a & dirs_b)
    
    return False


def extract_files(bead: Bead) -> set[str]:
    """
    Extract file paths from bead metadata.
    
    Sources (in priority order):
    1. bead.files - explicit field if present
    2. bead.title - parse patterns like "Add X to path/to/file.ts"
    3. bead.description - extract file paths mentioned
    4. bead.directory - fallback to directory scope
    """
    files = set()
    
    # Priority 1: Explicit files field
    if hasattr(bead, 'files') and bead.files:
        return set(bead.files)
    
    # Priority 2: Parse from title
    files.update(parse_file_paths(bead.title))
    
    # Priority 3: Parse from description
    if bead.description:
        files.update(parse_file_paths(bead.description))
    
    # Priority 4: Fallback to directory if no files found
    if not files and hasattr(bead, 'directory'):
        files.add(bead.directory)
    
    return files


def get_directory(file_path: str) -> str:
    """
    Extract top-level directory for scope comparison.
    
    Examples:
    - "src/api/auth.ts" → "src/api"
    - "lib/utils.py" → "lib"
    - "README.md" → "."
    """
    parts = file_path.split('/')
    if len(parts) <= 1:
        return '.'
    return '/'.join(parts[:-1])
```

## Overlap Detection

### File-Level (Primary)

Same file = same group. Always sequential.

| Bead A Files | Bead B Files | Overlap? |
|--------------|--------------|----------|
| `src/api.ts` | `src/api.ts` | ✅ Yes |
| `src/api.ts` | `src/db.ts` | ❌ No |
| `src/api.ts`, `lib/util.ts` | `lib/util.ts` | ✅ Yes |

### Directory-Level (Fallback)

Only used when file paths are unclear or missing.

| Bead A Dir | Bead B Dir | Overlap? |
|------------|------------|----------|
| `src/api` | `src/api` | ✅ Yes |
| `src/api` | `src/db` | ❌ No |
| `src/api` | `lib/utils` | ❌ No |

## Threshold Rules

```python
def should_parallelize(tracks: list[Track]) -> bool:
    """
    Threshold: ≥2 non-overlapping groups → parallel execution.
    """
    return len(tracks) >= 2
```

| Independent Groups | Action |
|--------------------|--------|
| 0 | Sequential (all overlap) |
| 1 | Sequential (single track) |
| ≥2 | **Parallel** (route to orchestrator) |

## Examples

### Example 1: Clear Separation

**Beads:**
1. "Add login endpoint" → `src/api/auth.ts`
2. "Add signup endpoint" → `src/api/auth.ts`
3. "Create user model" → `src/db/models/user.ts`
4. "Add validation utils" → `lib/validation.ts`

**Grouping:**
```
Track 1: [bead-1, bead-2]  # src/api/auth.ts (sequential)
Track 2: [bead-3]          # src/db/models/user.ts
Track 3: [bead-4]          # lib/validation.ts
```

**Result:** 3 tracks → PARALLEL_DISPATCH

### Example 2: Heavy Overlap

**Beads:**
1. "Add feature A" → `src/app.ts`
2. "Add feature B" → `src/app.ts`
3. "Add feature C" → `src/app.ts`, `lib/utils.ts`
4. "Add feature D" → `lib/utils.ts`

**Grouping:**
```
Track 1: [bead-1, bead-2, bead-3, bead-4]  # All overlap via app.ts or utils.ts
```

**Result:** 1 track → SINGLE_AGENT

### Example 3: Directory Fallback

**Beads:**
1. "Implement API routes" → (no files, dir: `src/api`)
2. "Add database migrations" → (no files, dir: `src/db`)
3. "Update API tests" → (no files, dir: `src/api`)

**Grouping:**
```
Track 1: [bead-1, bead-3]  # src/api (directory overlap)
Track 2: [bead-2]          # src/db
```

**Result:** 2 tracks → PARALLEL_DISPATCH

## Integration

### Called From

- `implement.md` Phase 2b (Execution Routing)
- Auto-routing when ≥2 independent beads detected

### Output

Returns track grouping for auto-trigger display:

```
📊 Parallel execution detected:
- Track 1: 2 tasks (src/api/auth.ts)
- Track 2: 1 task (src/db/models/)
- Track 3: 1 task (lib/)

⚡ Spawning workers...
```

## Related

- [implement.md](workflows/implement.md) - Execution routing

> 💡 **For orchestrator details:** Load `orchestrator` skill, then see:
> - `references/auto-routing.md` - Detection flow
> - `references/workflow.md` - Orchestrator workflow
