/**
 * maestro update -- alias for self-update.
 */

import { defineCommand } from 'citty';
import { runSelfUpdate } from './self.ts';

export default defineCommand({
  meta: { name: 'update', description: 'Update maestro to latest version (alias for self-update)\n\nExamples:\n  maestro update\n  maestro update --json' },
  args: {},
  run: runSelfUpdate,
});
