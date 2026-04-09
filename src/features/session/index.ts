/**
 * Public surface for the session feature.
 *
 * Cross-feature consumers (composition root, handoff use-case, tests)
 * import from `@/features/session`. Deep paths into the feature are not
 * allowed from outside (enforced by `bun run check:boundaries`).
 */
export type { AgentSession, AgentSlug } from "./domain/types.js";
export type { SessionDetectPort } from "./ports/session-detect.port.js";
export { ClaudeSessionDetectAdapter } from "./adapters/session-detect.adapter.js";
export {
  detectSession,
  type DetectSessionOpts,
  type DetectSessionResult,
} from "./usecases/detect-session.usecase.js";
export { registerSessionCommand } from "./commands/session.command.js";
