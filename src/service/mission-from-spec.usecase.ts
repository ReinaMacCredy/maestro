import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import { parseSpecFile } from "../repo/fs-spec-store.adapter.js";
import type { Mission } from "../types/mission.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface MissionFromSpecDeps {
  readonly repoRoot: string;
  readonly missionStore: MissionStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export class MissionRequiresHeavyModeError extends Error {
  readonly slug: string;
  readonly mode: string;
  constructor(slug: string, mode: string) {
    super(
      `Spec ${slug} has mode: ${mode}; mission from-spec requires mode: heavy. Light-mode specs go straight to task from-spec.`,
    );
    this.name = "MissionRequiresHeavyModeError";
    this.slug = slug;
    this.mode = mode;
  }
}

export async function missionFromSpec(
  deps: MissionFromSpecDeps,
  specPathArg: string,
): Promise<Mission> {
  const path = isAbsolute(specPathArg) ? specPathArg : resolve(deps.repoRoot, specPathArg);
  const raw = await readFile(path, "utf8");
  const spec = parseSpecFile(raw, path);
  if (spec.frontmatter.mode !== "heavy") {
    throw new MissionRequiresHeavyModeError(spec.frontmatter.slug, spec.frontmatter.mode);
  }
  const title = extractTitle(spec.body) ?? spec.frontmatter.slug;
  const mission = await deps.missionStore.create({
    slug: spec.frontmatter.slug,
    title,
    state: "approved",
    spec_path: path,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      mission_id: mission.id,
      from_state: null,
      to_state: "approved",
      trigger_verb: "mission:from-spec",
    },
  );
  return mission;
}

function extractTitle(body: string): string | undefined {
  const match = body.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : undefined;
}
