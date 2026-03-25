/**
 * Search usecases -- orchestrate session search operations.
 * Thin layer between MCP handlers and the SearchPort.
 * Gracefully degrades when search is unavailable.
 */

import type { SearchPort } from '../../search/port.ts';

export interface SearchOpts {
  agent?: string;
  limit?: number;
  days?: number;
}

/**
 * Search past sessions for context. Returns empty array when port is null.
 */
export async function searchForContext(
  port: SearchPort | undefined,
  query: string,
  opts?: SearchOpts,
): Promise<unknown[]> {
  if (!port) return [];
  return port.searchSessions(query, opts);
}

/**
 * Find sessions related to a file path. Returns empty array when port is null.
 */
export async function findRelated(
  port: SearchPort | undefined,
  filePath: string,
  limit?: number,
): Promise<unknown[]> {
  if (!port) return [];
  return port.findRelatedSessions(filePath, limit);
}
