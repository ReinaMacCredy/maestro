import type { GitPort } from "../ports/git.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { Handoff, HandoffPlan, HandoffSession } from "../domain/types.js";
import { generateHandoffId } from "../domain/id.js";
import { MaestroError } from "../domain/errors.js";
import { readJson } from "../lib/fs.js";
import { warn } from "../lib/output.js";
import { join } from "node:path";

export interface CreateHandoffOpts {
  readonly plan: boolean;
  readonly sitrep: string;
  readonly quickstart: string;
  readonly message?: string;
  readonly dir: string;
}

export async function createHandoff(
  git: GitPort,
  cass: CassPort,
  sessionDetect: SessionDetectPort,
  store: HandoffStorePort,
  opts: CreateHandoffOpts,
): Promise<Handoff> {
  // Collect git state
  if (!(await git.isRepo(opts.dir))) {
    throw new MaestroError("Not a git repository", [
      `Run this command from inside a git repo`,
    ]);
  }
  const gitState = await git.getState(opts.dir);

  // Detect session
  let session: HandoffSession = {
    agent: "unknown",
    sessionId: "none",
    sourcePath: "",
    cassIndexed: false,
  };

  const detected = await sessionDetect.detect(opts.dir);
  if (detected) {
    session = { ...detected };

    // Try to index via CASS
    if (await cass.isAvailable()) {
      try {
        await cass.indexOnce([detected.sourcePath]);
        session = { ...session, cassIndexed: true };
      } catch {
        warn("CASS indexing failed, continuing without session index");
      }
    } else {
      warn("CASS not available. Install: brew install dicklesworthstone/tap/cass");
    }
  } else {
    warn("Could not auto-detect session. Handoff will proceed without session reference.");
  }

  // Read plan if requested
  let plan: HandoffPlan | undefined;
  if (opts.plan) {
    plan = await readJson<HandoffPlan>(join(opts.dir, ".maestro", "plan.json"));
  }

  // Generate ID from existing handoffs
  const existing = await store.list();
  const existingIds = existing.map((e) => e.handoff.id);
  const id = generateHandoffId(existingIds);

  const handoff: Handoff = {
    id,
    timestamp: new Date().toISOString(),
    message: opts.message ?? opts.sitrep.slice(0, 80),
    session,
    plan,
    sitrep: opts.sitrep,
    quickstart: opts.quickstart,
    git: gitState,
  };

  await store.create(handoff);
  return handoff;
}
