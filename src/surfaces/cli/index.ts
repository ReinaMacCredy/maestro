import { defineCommand, runMain } from 'citty';
import { setOutputMode } from '../../core/output.ts';
import { initServices } from '../../services.ts';
import { findProjectRoot } from '../../features/detection.ts';
import { subCommands } from './registry.generated.ts';
import { VERSION } from '../../version.ts';
const subCommandNames = Object.keys(subCommands);
const metaCommands = new Set(['init', 'self-update', 'update']);

const main = defineCommand({
  meta: {
    name: 'maestro',
    version: VERSION,
    description: 'Agent-optimized development orchestrator',
  },
  args: {
    json: {
      type: 'boolean',
      description: 'Output as JSON',
      default: false,
    },
    version: {
      type: 'boolean',
      alias: 'v',
      description: 'Show version',
      default: false,
    },
  },
  subCommands,
  setup({ args }) {
    if (args.json) {
      setOutputMode('json');
    }

    // Find the actual subcommand (first argv that matches a known subcommand name).
    // Avoids false positives like `agents-md --action init` matching `init`.
    const subCommand = process.argv.find(a => subCommandNames.includes(a));
    const isMetaCommand = subCommand != null && metaCommands.has(subCommand);
    if (!isMetaCommand) {
      const projectRoot = findProjectRoot(process.cwd());
      if (projectRoot) {
        initServices(projectRoot);
      }
    }
  },
  run({ args, rawArgs }) {
    const hasSubCommand = rawArgs.some(a => subCommandNames.includes(a));
    if (hasSubCommand) return;

    if (args.version) {
      console.log(VERSION);
      return;
    }
    console.log(`maestro ${VERSION} -- agent-optimized development orchestrator`);
    console.log('Run `maestro --help` for usage.');
  },
});

runMain(main);
