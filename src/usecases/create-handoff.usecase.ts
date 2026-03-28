import type { GitPort } from "../ports/git.port.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { GitState, Handoff, HandoffPlan, HandoffSession, MaestroConfig } from "../domain/types.js";
import { generateHandoffId } from "../domain/id.js";
import { MaestroError } from "../domain/errors.js";
import { validateHandoff } from "../domain/validators.js";
import { MAESTRO_DIR, NO_SESSION_ID, UNKNOWN_AGENT } from "../domain/defaults.js";
import { readJson } from "../lib/fs.js";
import { warn } from "../lib/output.js";
import { detectSession } from "./detect-session.usecase.js";
import { join } from "node:path";

export interface CreateHandoffOpts {
  readonly plan: boolean;
  readonly sitrep?: string;
  readonly quickstart?: string;
  readonly task?: string;
  readonly instructions?: string;
  readonly message?: string;
  readonly session?: string;
  readonly noSession?: boolean;
  readonly dir: string;
}

export async function createHandoff(
  git: GitPort,
  sessionDetect: SessionDetectPort,
  config: MaestroConfig,
  store: HandoffStorePort,
  opts: CreateHandoffOpts,
): Promise<Handoff> {
  if (!(await git.isRepo(opts.dir))) {
    throw new MaestroError("Not a git repository", [
      "Run this command from inside a git repo",
    ]);
  }

  const [gitState, sessionResult] = await Promise.all([
    git.getState(opts.dir),
    detectSession(sessionDetect, config, {
      cwd: opts.dir,
      sessionId: opts.session,
      noSession: opts.noSession,
    }),
  ]);

  if (!sessionResult && !opts.noSession) {
    throw new MaestroError("No session detected", [
      "Get your session ID first: maestro session -q",
      "Then: maestro handoff --session <id> ...",
      "Or skip with --skip-session",
    ]);
  }

  const session: HandoffSession = sessionResult?.session ?? {
    agent: UNKNOWN_AGENT,
    sessionId: NO_SESSION_ID,
    sourcePath: "",
  };

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
    ...(opts.instructions !== undefined
      ? { instructions: opts.instructions }
      : {}),
    git: gitState,
  };

  validateHandoff(handoff);
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
