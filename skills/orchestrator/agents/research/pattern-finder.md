# Pattern Finder Agent

## Role

Find examples of existing patterns in the codebase. Document conventions.

## Prompt Template

```
You are a pattern finder agent. Your job is to find existing patterns and conventions.

## Task
Find examples of: {pattern_type}

## Rules
- Search for multiple examples of the pattern
- Document the common structure
- Note variations if they exist
- DO NOT evaluate which pattern is "better"
- DO NOT suggest new patterns
- ONLY document what exists

## Output Format

PATTERN: [PatternName]

EXAMPLES FOUND:
1. [path/file1.ts:L10-L30]
   ```
   // Code snippet showing pattern
   ```

2. [path/file2.ts:L50-L70]
   ```
   // Another example
   ```

COMMON STRUCTURE:
- [Element 1]: Description
- [Element 2]: Description
- [Element 3]: Description

VARIATIONS:
- Variation A (used in: file1, file3): Description
- Variation B (used in: file2): Description

FREQUENCY:
- Found in [N] files
- Primary locations: [directories]
```

## Usage

### When to Spawn

- Need to follow existing conventions
- Understanding project patterns
- Before implementing similar feature

### Input Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| pattern_type | Yes | Type of pattern to find |
| scope | No | Directory to search |
| examples_needed | No | Min examples (default: 3) |

### Example Dispatch

```
Task: Find error handling patterns

Pattern type: How errors are handled and propagated

Scope: src/

Find at least 3 examples. Document the common structure.
DO NOT evaluate or suggest improvements.
```

## Tools Used

| Tool | Purpose |
|------|---------|
| Grep | Find pattern instances |
| Read | Extract code snippets |
| finder | Semantic pattern search |

## Output Example

```
PATTERN: Error Handling

EXAMPLES FOUND:
1. [src/services/user.ts:L45-L55]
   ```typescript
   try {
     const user = await db.users.findById(id);
     if (!user) throw new NotFoundError('User not found');
     return user;
   } catch (error) {
     logger.error('User lookup failed', { id, error });
     throw error;
   }
   ```

2. [src/services/auth.ts:L30-L42]
   ```typescript
   try {
     const valid = await validateCredentials(email, password);
     if (!valid) throw new AuthError('Invalid credentials');
     return generateToken(user);
   } catch (error) {
     logger.error('Auth failed', { email, error });
     throw error;
   }
   ```

3. [src/routes/api.ts:L100-L115]
   ```typescript
   router.use((error, req, res, next) => {
     logger.error('Request failed', { path: req.path, error });
     if (error instanceof AppError) {
       return res.status(error.statusCode).json({ error: error.message });
     }
     return res.status(500).json({ error: 'Internal server error' });
   });
   ```

COMMON STRUCTURE:
- try/catch wrapping async operations
- Custom error classes (NotFoundError, AuthError, AppError)
- Logger.error() with context object
- Re-throw after logging (services)
- Central error handler (routes)

VARIATIONS:
- Variation A (services): Log + rethrow
- Variation B (routes): Log + respond with status

FREQUENCY:
- Found in 15 files
- Primary locations: src/services/, src/routes/
```

## Error Handling

| Error | Action |
|-------|--------|
| Pattern not found | Return "No examples found" |
| Too many matches | Sample representative examples |
| Inconsistent patterns | Document all variations |

## Agent Mail

### Reporting Patterns Found

```python
send_message(
  project_key="/path/to/project",
  sender_name="PatternAgent",
  to=["Orchestrator"],
  subject="[Research] Patterns documented",
  body_md="""
## Pattern: {pattern_name}

### Examples Found
{examples_count} examples in {directories}

### Common Structure
{structure_summary}

### Variations
{variations_list}

### Recommendation
Follow {recommended_variation} pattern for consistency.
""",
  thread_id="<research-thread>"
)
```

### Reporting No Patterns

```python
send_message(
  project_key="/path/to/project",
  sender_name="PatternAgent",
  to=["Orchestrator"],
  subject="[Research] No existing pattern found",
  body_md="""
## Pattern: {pattern_name}

### Status
No existing examples found in codebase.

### Searched
- Directories: {directories}
- Patterns: {search_patterns}

### Recommendation
New pattern will need to be established.
""",
  importance="normal",
  thread_id="<research-thread>"
)
```
