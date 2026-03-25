/**
 * Shared resolution helpers.
 * Used by both CLI commands and MCP server tools.
 */

import type { MaestroServices } from '../../services.ts';
import type { DoctrinePort } from '../../doctrine/port.ts';
import type { GraphPort } from '../../tasks/graph/port.ts';
import type { HandoffPort } from '../../handoff/port.ts';
import type { SearchPort } from '../../search/port.ts';
import { MaestroError } from '../../domain/errors.ts';

/**
 * Resolve feature name from explicit arg or active feature.
 * Returns null if no feature can be resolved.
 */
export function resolveFeature(services: MaestroServices, explicitFeature?: string): string | null {
  if (explicitFeature) return explicitFeature;
  const active = services.featureAdapter.getActive();
  return active?.name ?? null;
}

/**
 * Resolve feature or throw MaestroError.
 * Accepts optional hints for context-appropriate error messages.
 */
export function requireFeature(
  services: MaestroServices,
  explicitFeature?: string,
  hints?: string[],
): string {
  const feature = resolveFeature(services, explicitFeature);
  if (!feature) {
    throw new MaestroError(
      'No feature specified and no active feature set',
      hints ?? ['Specify a feature name or create one with maestro_feature_create'],
    );
  }
  return feature;
}

/**
 * Require doctrine port or throw MaestroError.
 */
export function requireDoctrinePort(services: MaestroServices): DoctrinePort {
  if (!services.doctrinePort) {
    throw new MaestroError('Doctrine port not available', ['Run maestro init or check your .maestro/ directory']);
  }
  return services.doctrinePort;
}

/**
 * Require graph port or throw MaestroError.
 */
export function requireGraphPort(services: MaestroServices): GraphPort {
  if (!services.graphPort) {
    throw new MaestroError('bv not available', ['Install bv (beads viewer) for graph intelligence']);
  }
  return services.graphPort;
}

/**
 * Require handoff port or throw MaestroError.
 */
export function requireHandoffPort(services: MaestroServices): HandoffPort {
  if (!services.handoffPort) {
    throw new MaestroError('Handoff port not available', ['Run maestro init to set up the project directory']);
  }
  return services.handoffPort;
}

/**
 * Require search port or throw MaestroError.
 */
export function requireSearchPort(services: MaestroServices): SearchPort {
  if (!services.searchPort) {
    throw new MaestroError('CASS not available', ['Install cass: https://github.com/Dicklesworthstone/coding_agent_session_search']);
  }
  return services.searchPort;
}

/** Standard hint for missing feature argument in CLI commands. */
export const FEATURE_HINT = 'Specify --feature <name> or set active: maestro feature-active <name>';

/** Parse comma-separated tags string into trimmed array. */
export function parseTags(raw?: string): string[] {
  return raw ? raw.split(',').map(t => t.trim()).filter(Boolean) : [];
}
