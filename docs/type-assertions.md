# Type Assertions Documentation

This document lists all justified type assertions (`as any`, `as unknown as`, `@ts-ignore`) in the Maestro codebase.

## Justified Assertions

### src/features/state/usecases/state-since.usecase.ts

**Location**: `evidenceDetail()` function, line ~107

**Assertion**: `const payload = e.payload as any;`

**Justification**: The `EvidenceRow.payload` field is a discriminated union type (`EvidencePayload<K>`) that TypeScript does not automatically narrow based on the `kind` field in a switch statement. The function performs runtime type checks (`typeof`, `Array.isArray`) on all property accesses, making this safe. The alternative would be to create separate handler functions for each evidence kind, which would be significantly more verbose without adding type safety beyond what the runtime checks already provide.

**Safety measures**:
- Runtime type guard: `if (!payload || typeof payload !== "object") return undefined;`
- All property accesses use runtime type checks before use
- Switch statement ensures only valid evidence kinds are handled

## Generated Files (Excluded)

The following generated files contain type assertions but are excluded from this audit as they are auto-generated:

- `src/infra/domain/built-in-skill-templates.ts` - Generated from `skills/built-in/`
- `src/infra/domain/bundled-skill-templates.ts` - Generated from `skills/bundled/`

These files are regenerated via `bun run sync:skills` and `bun run sync:bundled-skills` respectively.

## Verification

To verify no unsafe assertions exist in source code (excluding generated files):

```bash
# Check for 'as any' (should return 0 or only documented cases)
grep -rn "as any" src/ --include="*.ts" | grep -v "bundled-skill-templates.ts" | grep -v "built-in-skill-templates.ts"

# Check for 'as unknown as' (should return 0 or only documented cases)
grep -rn "as unknown as" src/ --include="*.ts" | grep -v "bundled-skill-templates.ts" | grep -v "built-in-skill-templates.ts"

# Check for '@ts-ignore' (should return 0 or only documented cases)
grep -rn "@ts-ignore" src/ --include="*.ts"
```

## Policy

New type assertions should be avoided. If absolutely necessary:

1. Document the assertion in this file with full justification
2. Implement runtime type guards to ensure safety
3. Consider refactoring to avoid the assertion if possible
4. Get code review approval before merging

Type assertions are a last resort when:
- TypeScript's type narrowing is insufficient despite correct runtime behavior
- Working with complex discriminated unions that don't narrow properly
- Interfacing with untyped external libraries (prefer `unknown` and narrow)
