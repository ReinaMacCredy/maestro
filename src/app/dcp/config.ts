/**
 * Resolve DCP config with defaults from a single source of truth.
 */

import { DEFAULT_SETTINGS, type DcpSettings } from '../../core/settings.ts';

export function resolveDcpConfig(override?: DcpSettings) {
  return { ...DEFAULT_SETTINGS.dcp, ...override };
}
