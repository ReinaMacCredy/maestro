import type { GitPort } from "../ports/git.port.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { GitState, Handoff, HandoffPlan, HandoffSession } from "../domain/types.js";
import { generateHandoffId } from "../domain/id.js";
import { MaestroError } from "../domain/errors.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { readJson } from "../lib/fs.js";
import { warn } from "../lib/output.js";
import { join } from "node:path";

export interface CreateHandoffOpts {
  readonly plan: boolean;
  readonly sitrep?: string;
  readonly quickstart?: string;
  readonly task?: string;
  readonly message?: string;
  readonly dir: string;
}

export async function createHandoff(
  git: GitPort,
  sessionDetect: SessionDetectPort,
  store: HandoffStorePort,
  opts: CreateHandoffOpts,
): Promise<Handoff> {
  if (!(await git.isRepo(opts.dir))) {
    throw new MaestroError("Not a git repository", [
      "Run this command from inside a git repo",
    ]);
  }

  const [gitState, detected] = await Promise.all([
    git.getState(opts.dir),
    sessionDetect.detect(opts.dir),
  ]);

  const session: HandoffSession = detected ?? {
    agent: "unknown",
    sessionId: "none",
    sourcePath: "",
    cassIndexed: false,
  };

  if (!detected) {
    warn("Could not auto-detect session. Handoff will proceed without session reference.");
  }

  let plan: HandoffPlan | undefined;
  if (opts.plan) {
    plan = await readJson<HandoffPlan>(join(opts.dir, MAESTRO_DIR, "plan.json"));
  }

  const existingIds = await store.listIds();
  const id = generateHandoffId(existingIds);

  const sitrep = opts.sitrep ?? formatAutoSitrep(gitState);
  const quickstart = opts.quickstart ?? "See handoff briefing for orientation.";
  const message = opts.message
    ?? opts.task
    ?? sitrep.slice(0, 80);

  const handoff: Handoff = {
    id,
    timestamp: new Date().toISOString(),
    message,
    session,
    plan,
    sitrep,
    quickstart,
    git: gitState,
  };

  await store.create(handoff);
  return handoff;
}

function formatAutoSitrep(git: GitState): string {
  const lines = [`Branch: ${git.branch}`];

  if (git.recentCommits.length > 0) {
    lines.push("Recent commits:");
    for (const c of git.recentCommits.slice(0, 5)) {
      lines.push(`- ${c}`);
    }
  }

  lines.push(`Working tree: ${git.workingTreeClean ? "clean" : "dirty"}`);

  if (git.diffStat !== "+0 -0") {
    lines.push(`Diff: ${git.diffStat}`);
  }

  return lines.join("\n");
}
