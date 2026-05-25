---
name: maestro-dev
description: "Development workflow for maestroCLI itself. Encodes the hexagonal architecture pattern (port -> adapter -> use-case -> command -> MCP tool -> test) and project-specific conventions. Use when implementing new maestro features, adding CLI commands, extending the MCP server, creating new adapters, modifying ports, writing use-cases, or debugging maestro's own code. Also use when you need to understand how maestro's layers connect or where to put new code."
---

# maestroCLI Development Workflow

## Architecture

maestroCLI follows hexagonal architecture. Every feature touches the same layers in the same order:

```
commands/  -->  usecases/  -->  ports/  <--  adapters/
(CLI I/O)       (rules)        (interfaces)  (implementations)

server/  -->  usecases/  -->  ports/  <--  adapters/
(MCP I/O)      (rules)        (interfaces)  (implementations)
```

Commands and MCP server tools are thin I/O shells. Business logic lives in use-cases. Ports define what the system needs. Adapters provide it.

## Adding a New Feature: Step by Step

### 1. Define or Extend the Port Interface (`src/ports/`)

Ports are TypeScript interfaces that describe what the system needs without saying how to provide it. If your feature needs new persistence or external interaction, define it here.

```typescript
// src/ports/memory.ts
export interface MemoryPort {
  write(feature: string, name: string, content: string): Promise<void>;
  read(feature: string, name: string): Promise<string | undefined>;
  list(feature: string): Promise<string[]>;
  delete(feature: string, name: string): Promise<void>;
  compile(feature: string): Promise<string>;
}
```

**Rules:**
- Ports are pure interfaces -- no implementation, no imports from adapters
- Method signatures use domain types, not framework types
- Every method returns a Promise (even if the current adapter is sync)
- One port per domain concern (tasks, features, plans, memory)

### 2. Implement the Adapter (`src/adapters/`)

Adapters implement port interfaces against concrete backends (filesystem, beads_rust, etc.).

```typescript
// src/adapters/fs/memory.ts
export class FsMemoryAdapter implements MemoryPort {
  constructor(private directory: string) {}

  async write(feature: string, name: string, content: string): Promise<void> {
    const dir = join(this.directory, '.maestro', 'features', feature, 'memory');
    await ensureDir(dir);
    const filePath = join(dir, `${name}.md`);
    await writeFileAtomic(filePath, content);
  }
  // ... other methods
}
```

**Rules:**
- Adapter classes are named `Fs{Domain}Adapter` for filesystem, `Br{Domain}Adapter` for beads_rust
- Constructor takes `directory: string` (project root)
- Use `ensureDir()` before writes, `writeFileAtomic()` for atomic I/O (temp + rename)
- Path length: respect `MAX_PATH_LENGTH = 240`
- Adapters live in `src/adapters/` (flat files) or `src/adapters/fs/` (filesystem-specific)

### 3. Wire the Use-Case (`src/usecases/`)

Use-cases contain business logic. They receive ports via the services singleton and orchestrate operations.

```typescript
// src/usecases/check-status.ts
export async function checkStatus(
  featureAdapter: FeaturePort,
  taskPort: TaskPort,
  planAdapter: PlanPort,
  memoryAdapter: MemoryPort,
  directory: string,
  featureName?: string,
): Promise<FeatureStatus> {
  const feature = featureName
    ? await featureAdapter.get(featureName)
    : await featureAdapter.getActive();
  if (!feature) throw new MaestroError('No active feature', ['Run: maestro feature-active <name>']);
  // ... orchestrate across ports
}
```

**Rules:**
- Use-cases are pure functions (no classes), exported from their own file
- Parameters are port interfaces, not adapter instances (testable)
- Throw `MaestroError` with actionable `.hints[]` array
- One use-case per business operation
- Use-cases never import from `src/commands/` or `src/server/`

### 4. Add the CLI Command (`src/commands/<noun>/<verb>.ts`)

Commands are organized as noun/verb directories using citty's `defineCommand`.

```typescript
// src/commands/memory/write.ts
import { defineCommand } from 'citty';
import { getServices } from '../../services.ts';
import { output } from '../../lib/output.ts';

export default defineCommand({
  meta: { name: 'memory-write', description: 'Write a memory file for a feature' },
  args: {
    feature: { type: 'string', required: true, description: 'Feature name' },
    name: { type: 'string', required: true, description: 'Memory file name' },
    content: { type: 'string', required: true, description: 'Content to write' },
    json: { type: 'boolean', default: false, description: 'Output as JSON' },
  },
  async run({ args }) {
    const { memoryAdapter } = getServices();
    await memoryAdapter.write(args.feature, args.name, args.content);
    output(args.json, { feature: args.feature, name: args.name }, (r) => [
      `[ok] Wrote memory: ${r.name} for feature: ${r.feature}`,
    ]);
  },
});
```

**Rules:**
- File path = `src/commands/{noun}/{verb}.ts` (e.g., `memory/write.ts`)
- CLI name = `{noun}-{verb}` (e.g., `memory-write`)
- Always include `json` boolean arg for dual-mode output
- Use `getServices()` to access ports -- never instantiate adapters directly
- Use `output(isJson, data, textFormatter)` for all output
- Error handling: let `MaestroError` propagate -- the root command catches it

### 5. Register the MCP Server Tool (`src/server/`)

CLI commands are the primary surface. Each command calls a shared use-case function.

```typescript
// In src/surfaces/cli/commands/memory-write.ts
export const memoryWrite = defineCommand({
  name: 'memory-write',
  description: 'Write a memory file for a feature',
  args: {
    feature: { type: 'string', describe: 'Feature name' },
    name: { type: 'string', describe: 'Memory file name', required: true },
    file: { type: 'string', describe: 'Path to content file', required: true },
  },
}, async (args, services) => {
  const content = readFileSync(args.file, 'utf-8');
  await services.memoryAdapter.write(args.feature, args.name, content);
  return { path: `Written: ${args.name}` };
});
```

**Rules:**
- Command name: `kebab-case` (hyphens, not underscores)
- Args use yargs-style type declarations
- All commands accept `--json` for structured output
- Share the same use-case logic as the CLI command

### 6. Add Tests (`src/__tests__/`)

```typescript
// src/__tests__/unit/memory.test.ts
import { describe, it, expect } from 'bun:test';

describe('FsMemoryAdapter', () => {
  it('writes and reads a memory file', async () => {
    const adapter = new FsMemoryAdapter(tmpDir);
    await adapter.write('my-feature', 'notes', 'hello world');
    const content = await adapter.read('my-feature', 'notes');
    expect(content).toBe('hello world');
  });
});
```

**Rules:**
- Unit tests in `src/__tests__/unit/` -- test adapters and use-cases
- Integration tests in `src/__tests__/integration/` -- test CLI commands
- Use `bun:test` (describe, it, expect)
- Create temp directories for filesystem tests
- Mock external dependencies, not internal modules

### 7. Build and Verify

```bash
bun run build    # Runs generators (skills registry, command registry) + bundles
bun test src/    # Runs all tests
```

Build must pass before proceeding. Tests must pass before committing.

## Service Wiring

All ports are wired in `src/services.ts` via a module-level singleton:

```typescript
// Root command calls this once
initServices(directory);

// All commands and MCP tools call this
const { taskPort, memoryAdapter, featureAdapter, planAdapter } = getServices();
```

The task backend is selected by config: `configAdapter.get().taskBackend` chooses between `FsTaskAdapter` (default) and `BrTaskAdapter`.

## Error Handling Pattern

```typescript
throw new MaestroError(
  'Feature not found',                              // Title
  ['Run: maestro feature-list to see available features']  // Actionable hints
);
```

Hints are printed line-by-line after the error title. Every error should tell the user what to do next.

## State Machine Pattern

Task and feature states use explicit transition maps:

```typescript
const VALID_TRANSITIONS: Record<TaskStatus, TaskStatus[]> = {
  open: ['claimed'],
  claimed: ['done', 'blocked'],
  blocked: ['open'],
  done: [],
};
```

Always validate transitions before applying them. Invalid transitions throw `MaestroError`.

## Anti-Patterns

- **Do not** import adapters in commands -- use `getServices()`
- **Do not** put business logic in commands -- extract to use-cases
- **Do not** use `console.log` -- use `output()` for dual-mode JSON/text
- **Do not** write files without `ensureDir()` first
- **Do not** create new ports when an existing one can be extended
- **Do not** skip the build step -- generators produce required files
