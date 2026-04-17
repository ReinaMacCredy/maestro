# General Code Style Guide

## Naming
- Use descriptive, intention-revealing names
- Avoid abbreviations unless universally understood (e.g., `url`, `id`, `db`)
- Constants: UPPER_SNAKE_CASE
- Boolean variables: prefix with `is`, `has`, `should`, `can`

## Functions
- Single responsibility: one function does one thing
- Max 30 lines preferred; extract if longer
- Pure functions preferred: same input always produces same output
- Early returns to reduce nesting

## Error Handling
- Fail fast and loud at boundaries (user input, external APIs)
- Trust internal code -- no defensive checks for impossible states
- Provide actionable error messages with context
- Never swallow errors silently

## Comments
- Code should be self-documenting; comments explain "why", not "what"
- Delete commented-out code -- version control exists
- TODO format: `TODO(author): description`

## Testing
- Test behavior, not implementation details
- One assertion concept per test
- Descriptive test names that read as specifications
- Arrange-Act-Assert pattern

## Security
- Validate all external input at system boundaries
- Never log secrets, tokens, or passwords
- Sanitize output to prevent injection
- Use parameterized queries for database access

## Dependencies
- Prefer standard library over external dependencies
- Evaluate maintenance status before adding new dependencies
- Pin versions for reproducible builds
