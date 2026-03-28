import type { GitPort } from "../ports/git.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { SessionDetectPort } from "../ports/session-detect.port.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import type { Handoff, HandoffPlan, HandoffSession } from "../domain/types.js";
import { generateHandoffId } from "../domain/id.js";
import { MaestroError } from "../domain/errors.js";
import { CASS_INSTALL_HINT, MAESTRO_DIR } from "../domain/defaults.js";
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
  if (!(await git.isRepo(opts.dir))) {
    throw new MaestroError("Not a git repository", [
      "Run this command from inside a git repo",
    ]);
  }

  const [gitState, detected] = await Promise.all([
    git.getState(opts.dir),
    sessionDetect.detect(opts.dir),
  ]);

  let session: HandoffSession = {
    agent: "unknown",
    sessionId: "none",
    sourcePath: "",
    cassIndexed: false,
  };

  if (detected) {
    session = { ...detected };

    if (await cass.isAvailable()) {
      try {
        await cass.indexOnce([detected.sourcePath]);
        session = { ...session, cassIndexed: true };
      } catch {
        warn("CASS indexing failed, continuing without session index");
      }
    } else {
      warn(`CASS not available. ${CASS_INSTALL_HINT}`);
    }
  } else {
    warn("Could not auto-detect session. Handoff will proceed without session reference.");
  }

  let plan: HandoffPlan | undefined;
  if (opts.plan) {
    plan = await readJson<HandoffPlan>(join(opts.dir, MAESTRO_DIR, "plan.json"));
  }

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
