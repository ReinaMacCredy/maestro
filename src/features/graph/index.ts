/**
 * Public surface for the graph feature.
 *
 * Cross-feature consumers (`src/tui/state/snapshot.ts`,
 * `src/features/memory/usecases/memory-stats.usecase.ts`, the composition
 * root, and tests) import from `@/features/graph`. Deep paths into the
 * feature are not allowed from outside (enforced by
 * `bun run check:boundaries`).
 */
export type {
  GraphRelation,
  ProjectEdge,
  ProjectGraph,
  ProjectNode,
} from "./domain/types.js";
export type { ProjectGraphStorePort } from "./ports/project-graph-store.port.js";
export { FsProjectGraphStoreAdapter } from "./adapters/project-graph-store.adapter.js";
export {
  getGraphContext,
  type GraphContext,
  type ProjectRelationship,
} from "./usecases/graph-context.usecase.js";
export {
  linkProjects,
  type LinkOpts,
  type LinkResult,
} from "./usecases/graph-link.usecase.js";
export { registerGraphLinkCommand } from "./commands/graph-link.command.js";
export { registerGraphContextCommand } from "./commands/graph-context.command.js";
