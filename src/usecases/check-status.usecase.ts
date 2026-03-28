import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { StatusReport } from "../domain/types.js";

export async function checkStatus(
  store: HandoffStorePort,
  config: ConfigPort,
  cass: CassPort,
  git: GitPort,
  dir: string,
): Promise<StatusReport> {
  const [
    pendingHandoffs,
    projectConfigExists,
    globalConfigExists,
    cassAvailable,
    gitAvailable,
  ] = await Promise.all([
    store.list({ status: "pending" }),
    config.exists("project", dir),
    config.exists("global", dir),
    cass.isAvailable(),
    git.isRepo(dir),
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
    cassAvailable,
    gitAvailable,
  };
}
