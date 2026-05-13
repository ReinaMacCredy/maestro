import { MaestroError } from "@/shared/errors.js";

export interface McpToolError {
  readonly code: string;
  readonly message: string;
  readonly hints: readonly string[];
  /** Single offending argument name (e.g. "taskId") for INVALID_ARG. */
  readonly arg?: string;
}

export interface McpToolSuccess<T = unknown> {
  readonly ok: true;
  readonly data: T;
}

export interface McpToolFailure {
  readonly ok: false;
  readonly error: McpToolError;
}

export type McpToolResult<T = unknown> = McpToolSuccess<T> | McpToolFailure;

export type CallToolResult = {
  content: { type: "text"; text: string }[];
  structuredContent?: Record<string, unknown>;
  isError?: boolean;
};

export function ok<T = unknown>(data: T): McpToolSuccess<T> {
  return { ok: true, data };
}

export interface FailOptions {
  readonly hints?: readonly string[];
  readonly arg?: string;
}

export function fail(
  code: string,
  message: string,
  options: FailOptions = {},
): McpToolFailure {
  const hints = options.hints ?? [];
  return {
    ok: false,
    error: { code, message, hints, ...(options.arg !== undefined ? { arg: options.arg } : {}) },
  };
}

export function fromMaestroError(err: unknown, fallbackCode = "MAESTRO_ERROR"): McpToolFailure {
  if (err instanceof MaestroError) {
    // Prefer the explicit code attached at throw time. Domain factories that
    // pre-date this convention fall back to message-pattern heuristics so we
    // don't lose code routing for legacy throw sites.
    const code = err.code ?? deriveErrorCode(err.message, fallbackCode);
    return fail(code, err.message, { hints: err.hints });
  }
  if (err instanceof Error) {
    return fail(fallbackCode, err.message);
  }
  return fail(fallbackCode, String(err));
}

function deriveErrorCode(message: string, fallback: string): string {
  const lower = message.toLowerCase();
  // Order matters: most-specific patterns first, since later checks would
  // otherwise shadow them (e.g. "already completed" must map to
  // ALREADY_COMPLETED, not ALREADY_EXISTS).
  if (lower.includes("not found")) return "NOT_FOUND";
  if (lower.includes("completed")) return "ALREADY_COMPLETED";
  if (lower.includes("cycle")) return "CYCLE_DETECTED";
  if (lower.includes("self-block")) return "SELF_BLOCK";
  if (lower.includes("ownership") || lower.includes("owned by")) return "OWNERSHIP_CONFLICT";
  if (lower.includes("already")) return "ALREADY_EXISTS";
  if (lower.includes("contract")) return "CONTRACT_ERROR";
  if (lower.includes("policy")) return "POLICY_ERROR";
  return fallback;
}

export function toCallToolResult<T = unknown>(result: McpToolResult<T>): CallToolResult {
  if (result.ok) {
    return {
      content: [{ type: "text", text: JSON.stringify(result.data, null, 2) }],
      structuredContent: result.data as Record<string, unknown>,
    };
  }
  const errorPayload: Record<string, unknown> = {
    code: result.error.code,
    message: result.error.message,
  };
  if (result.error.hints.length > 0) {
    errorPayload.hints = result.error.hints;
  }
  if (result.error.arg !== undefined) {
    errorPayload.arg = result.error.arg;
  }
  return {
    content: [{ type: "text", text: JSON.stringify(errorPayload) }],
    isError: true,
  };
}
