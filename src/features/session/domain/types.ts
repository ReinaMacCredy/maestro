/** Supported agent identifiers. */
export type AgentSlug =
  | "claude-code"
  | "codex"
  | "gemini"
  | "opencode"
  | "amp"
  | "cline"
  | "aider"
  | "cursor"
  | (string & {});

/**
 * Phase 1 strip: AgentSession replaces the old HandoffSession shape.
 * The conductor no longer owns handoff records; this type describes the
 * identity the session-detect adapter emits so memory + notes can
 * associate artifacts with the current shell.
 */
export interface AgentSession {
  readonly agent: AgentSlug;
  readonly sessionId: string;
  readonly sourcePath: string;
  readonly startedAt?: number;
}
