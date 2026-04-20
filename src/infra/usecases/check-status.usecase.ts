import type { ConfigPort } from "../ports/config.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { StatusReport } from "@/infra/domain/status-types.js";
import { countLegacyHandoffFiles } from "@/features/handoff";

export async function checkStatus(
  config: ConfigPort,
  git: GitPort,
  dir: string,
): Promise<StatusReport> {
  const [
    projectConfigExists,
    globalConfigExists,
    gitAvailable,
    legacyHandoffCount,
  ] = await Promise.all([
    config.exists("project", dir),
    config.exists("global", dir),
    git.isRepo(dir),
    countLegacyHandoffFiles(dir),
  ]);

  const configSource: StatusReport["configSource"] = projectConfigExists
    ? "project"
    : globalConfigExists
      ? "global"
      : "none";

  return {
    initialized: projectConfigExists || globalConfigExists,
    configSource,
    gitAvailable,
    legacyHandoffCount,
  };
}
