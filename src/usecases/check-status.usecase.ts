import type { ConfigPort } from "../ports/config.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { StatusReport } from "../domain/types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";

/**
 * Phase 1 strip: the conductor model does not own handoffs or CASS
 * availability. Pending handoffs remain on the struct as an empty
 * array until Phase 2 introduces the UKI handoff store with a new
 * shape, and CASS availability is hardcoded false until Phase 2
 * removes the field entirely.
 */
export async function checkStatus(
  config: ConfigPort,
  git: GitPort,
  handoffStore: HandoffStorePort,
  dir: string,
): Promise<StatusReport> {
  const [
    projectConfigExists,
    globalConfigExists,
    gitAvailable,
    pendingHandoffs,
  ] = await Promise.all([
    config.exists("project", dir),
    config.exists("global", dir),
    git.isRepo(dir),
    handoffStore.list({ status: "pending" }),
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
