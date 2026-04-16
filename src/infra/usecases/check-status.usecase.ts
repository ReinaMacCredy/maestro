import type { ConfigPort } from "../ports/config.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { PendingHandoffSummary, StatusReport } from "@/infra/domain/status-types.js";
import type { HandoffStorePort, UkiHandoff } from "@/features/handoff";

/**
 * Pending handoffs stay opt-in for status JSON, but they are projected to
 * the stable summary shape that existing CLI consumers expect.
 */
export interface CheckStatusOptions {
  readonly includePendingHandoffs?: boolean;
}

export async function checkStatus(
  config: ConfigPort,
  git: GitPort,
  handoffStore: HandoffStorePort,
  dir: string,
  options: CheckStatusOptions = {},
): Promise<StatusReport> {
  const pendingHandoffsPromise = options.includePendingHandoffs === true
    ? handoffStore.list({ status: "pending" })
    : Promise.resolve([]);

  const [
    projectConfigExists,
    globalConfigExists,
    gitAvailable,
    pendingHandoffs,
  ] = await Promise.all([
    config.exists("project", dir),
    config.exists("global", dir),
    git.isRepo(dir),
    pendingHandoffsPromise,
  ]);

  const configSource: StatusReport["configSource"] = projectConfigExists
    ? "project"
    : globalConfigExists
      ? "global"
      : "none";

  return {
    initialized: projectConfigExists || globalConfigExists,
    configSource,
    pendingHandoffs: pendingHandoffs.map(toPendingHandoffSummary),
    gitAvailable,
  };
}

function toPendingHandoffSummary(handoff: UkiHandoff): PendingHandoffSummary {
  return {
    id: handoff.id,
    agent: handoff.agent,
    createdAt: handoff.timestamp,
  };
}
