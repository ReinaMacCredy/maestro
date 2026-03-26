import { defineCommand, runMain } from 'citty';
import { setOutputMode } from '../../infra/utils/output.ts';
import { initServices } from '../../services.ts';
import { findProjectRoot } from '../../infra/adapters/features/detection.ts';
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

    // Grouped command summary -- more useful than flat --help for 70+ commands
    console.log(`maestro ${VERSION} -- agent-optimized development orchestrator\n`);
    console.log('Command groups:');
    const groups: [string, string][] = [
      ['feature-*', 'Feature lifecycle (create, complete, list, info, active)'],
      ['plan-*', 'Plan management (write, approve, revoke, read, comment)'],
      ['task-*', 'Task operations (sync, claim, done, block, list, next, info, spec, report)'],
      ['memory-*', 'Memory read/write (write, read, list, delete, compile, consolidate, promote)'],
      ['doctrine-*', 'Doctrine rules (write, read, list, approve, deprecate, suggest)'],
      ['handoff-*', 'Agent handoffs (send, receive, ack)'],
      ['config-*', 'Settings (get, set, agent)'],
      ['graph-*', 'Dependency graph (insights, next, plan)'],
      ['search-*', 'Session search (sessions, related)'],
      ['toolbox-*', 'Tool management (add, create, install, list, remove, test)'],
      ['skill*', 'Built-in skills (skill, skill-list)'],
      ['visual*', 'HTML visualizations (visual, debug-visual)'],
    ];
    for (const [prefix, desc] of groups) {
      console.log(`  ${prefix.padEnd(14)} ${desc}`);
    }
    console.log(`\nOther: init, status, ping, doctor, history, execution-insights, dcp-preview, agents-md`);
    console.log(`\nRun \`maestro <command> --help\` for details on any command.`);
    console.log('All commands accept --json for structured output.');
  },
});

runMain(main);
