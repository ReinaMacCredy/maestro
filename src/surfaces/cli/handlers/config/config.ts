/**
 * maestro config-agent -- get agent-specific config.
 */

import { defineCommand } from 'citty';
import { getServices } from '../../../../services.ts';
import { output } from '../../../../infra/utils/output.ts';
import { MaestroError, handleCommandError } from '../../../../domain/errors.ts';
import { AGENT_NAMES } from '../../../../domain/types.ts';
import type { AgentName } from '../../../../domain/types.ts';

export default defineCommand({
  meta: { name: 'config-agent', description: 'Get agent-specific config\n\nExamples:\n  maestro config-agent --agent forager-worker\n  maestro config-agent --agent hive-master --json' },
  args: {
    agent: {
      type: 'string',
      description: 'Agent name (hive-master, architect-planner, swarm-orchestrator, scout-researcher, forager-worker, hygienic-reviewer)',
      required: true,
    },
  },
  async run({ args }) {
    try {
      if (!AGENT_NAMES.includes(args.agent as AgentName)) {
        throw new MaestroError(`unknown agent '${args.agent}'`, [`Valid agents: ${AGENT_NAMES.join(', ')}`]);
      }

      const { settingsPort } = getServices();
      const agentConfig = (settingsPort as any).getAgentConfig(args.agent);
      output(agentConfig, (c) => JSON.stringify(c, null, 2));
    } catch (err) {
      handleCommandError('config-agent', err);
    }
  },
});
