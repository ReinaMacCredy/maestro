/**
 * Resolve doctrine config with defaults from a single source of truth.
 */

import { DEFAULT_SETTINGS, type DoctrineSettings } from '../../core/settings.ts';

export function resolveDoctrineConfig(override?: DoctrineSettings) {
  return { ...DEFAULT_SETTINGS.doctrine, ...override };
}
