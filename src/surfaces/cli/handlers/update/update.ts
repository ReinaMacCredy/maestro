/**
 * maestro update -- alias for self-update.
 */

import { defineCommand } from 'citty';
import { runSelfUpdate } from './self.ts';

export default defineCommand({
  meta: { name: 'update', description: 'Update maestro to latest version (alias for self-update)' },
  args: {},
  run: runSelfUpdate,
});
