/**
 * MCP response formatting utilities.
 * Forked from packages/claude-hive/src/utils/respond.ts
 */

import { MaestroError } from '../../core/errors.ts';

type McpResponse = { content: Array<{ type: 'text'; text: string }> };

/** Wrap an MCP tool handler with standard MaestroError / unexpected error handling. */
export function withErrorHandling<T>(
  fn: (input: T) => Promise<McpResponse>,
): (input: T) => Promise<McpResponse> {
  return async (input) => {
    try {
      return await fn(input);
    } catch (err) {
      if (err instanceof MaestroError) {
        return errorResponse({ terminal: false, reason: 'maestro_error', error: err.message, suggestions: err.hints });
      }
      return errorResponse({ terminal: true, reason: 'unexpected_error', error: String(err) });
    }
  };
}

const MAX_RESPONSE_BYTES = 102400; // 100KB

/** Recursively strip keys with null or undefined values from plain objects. */
function stripNulls(obj: unknown): unknown {
  if (obj === null || obj === undefined) return obj;
  if (Array.isArray(obj)) return obj.map(stripNulls);
  if (typeof obj === 'object') {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      if (v !== null && v !== undefined) out[k] = stripNulls(v);
    }
    return out;
  }
  return obj;
}

/**
 * Format a payload as an MCP text content response.
 * Strips null/undefined fields, uses compact JSON, handles circular refs and truncation.
 */
export function respond(payload: Record<string, unknown>): { content: Array<{ type: 'text'; text: string }> } {
  let text: string;
  try {
    text = JSON.stringify(stripNulls(payload));
  } catch (err) {
    text = JSON.stringify({
      success: false,
      terminal: true,
      reason: 'serialization_error',
      error: `Failed to serialize response: ${err instanceof Error ? err.message : String(err)}`,
    });
  }

  if (text.length > MAX_RESPONSE_BYTES) {
    text = JSON.stringify({
      success: false,
      truncated: true,
      original_keys: Object.keys(payload),
      truncation_message: `Response exceeded ${MAX_RESPONSE_BYTES} byte limit (was ${text.length} bytes). Use more specific queries to reduce output size.`,
    });
  }

  return { content: [{ type: 'text' as const, text }] };
}

/** Format a plain text MCP response. */
export function textResponse(text: string): { content: Array<{ type: 'text'; text: string }> } {
  return { content: [{ type: 'text', text }] };
}

export interface ErrorResponseOptions {
  terminal: boolean;
  reason: string;
  error: string;
  hint?: string;
  suggestions?: string[];
  [key: string]: unknown;
}

const ERROR_RESPONSE_KNOWN_KEYS = ['terminal', 'reason', 'error', 'hint', 'suggestions'];

/** Produce a standard MCP error response with success: false. */
export function errorResponse(opts: ErrorResponseOptions) {
  return respond({
    success: false,
    terminal: opts.terminal,
    reason: opts.reason,
    error: opts.error,
    ...(opts.hint !== undefined && { hint: opts.hint }),
    ...(opts.suggestions !== undefined && opts.suggestions.length > 0 && { suggestions: opts.suggestions }),
    ...Object.fromEntries(
      Object.entries(opts).filter(([k]) => !ERROR_RESPONSE_KNOWN_KEYS.includes(k))
    ),
  });
}
