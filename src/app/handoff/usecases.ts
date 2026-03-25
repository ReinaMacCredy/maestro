/**
 * Handoff usecases -- orchestrate handoff operations.
 * Thin layer between MCP handlers and the HandoffPort.
 */

import type { HandoffPort, HandoffDocument, HandoffResult } from '../../handoff/port.ts';

export interface BuildAndSendOpts {
  targetAgent?: string;
  additionalContext?: string;
  goal?: string;
}

/**
 * Build a handoff document and send it in one step.
 * Orchestrates port.buildHandoff() + port.sendHandoff().
 */
export async function buildAndSendHandoff(
  port: HandoffPort,
  feature: string,
  taskId: string,
  opts?: BuildAndSendOpts,
): Promise<{ handoff: HandoffDocument; result: HandoffResult }> {
  const handoff = await port.buildHandoff(feature, taskId);

  if (opts?.additionalContext) {
    handoff.criticalContext = opts.additionalContext;
  }
  if (opts?.goal) {
    handoff.goal = opts.goal;
  }

  const result = await port.sendHandoff(feature, handoff, opts?.targetAgent);
  return { handoff, result };
}
