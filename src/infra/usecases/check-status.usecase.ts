import type { ConfigPort } from "../ports/config.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { StatusReport } from "@/infra/domain/status-types.js";
import type { HandoffStorePort } from "@/features/handoff";

/**
 * Pending handoffs stay in their persisted record shape so CLI JSON
 * consumers see the same contract the store exposes.
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
  const pendingHandoffsPromise = options.includePendingHandoffs === false
    ? Promise.resolve([])
    : handoffStore.list({ status: "pending" });

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
    pendingHandoffs,
    cassAvailable: false,
    gitAvailable,
  };
}
