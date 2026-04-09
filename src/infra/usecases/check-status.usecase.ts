import type { ConfigPort } from "../ports/config.port.js";
import type { GitPort } from "../ports/git.port.js";
import type { PendingHandoffSummary, StatusReport } from "@/infra/domain/status-types.js";
import type { HandoffStorePort, UkiHandoff } from "@/features/handoff";

/**
 * Phase 7: `StatusReport.pendingHandoffs` is a narrow summary projection
 * of UkiHandoff records. The full records still live in the handoff
 * store and the TUI reads them directly through the handoff port; this
 * usecase is only responsible for the CLI `status --json` view.
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
    pendingHandoffs: pendingHandoffs.map(toPendingHandoffSummary),
    cassAvailable: false,
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
