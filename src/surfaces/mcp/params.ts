/**
 * Shared Zod parameter builders for MCP tool schemas.
 * Reduces duplication across server registration files.
 */

import { z } from 'zod';

export const featureParam = () =>
  z.string().min(1).optional().describe('Feature name (defaults to active feature)');

export const taskParam = () =>
  z.string().describe('Task folder ID');

export const limitParam = (def: number) =>
  z.number().optional().default(def).describe(`Max results (default: ${def})`);
