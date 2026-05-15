import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { ExecPlanStorePort } from "../repo/exec-plan-store.port.js";
import { parseSpecFile } from "../repo/fs-spec-store.adapter.js";
import type { ExecPlan } from "../types/exec-plan.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface PlanFromSpecDeps {
  readonly repoRoot: string;
  readonly planStore: ExecPlanStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export class PlanRequiresHeavyModeError extends Error {
  readonly slug: string;
  readonly mode: string;
  constructor(slug: string, mode: string) {
    super(
      `Spec ${slug} has mode: ${mode}; plan from-spec requires mode: heavy. Light-mode specs go straight to task from-spec.`,
    );
    this.name = "PlanRequiresHeavyModeError";
    this.slug = slug;
    this.mode = mode;
  }
}

export async function planFromSpec(
  deps: PlanFromSpecDeps,
  specPathArg: string,
): Promise<ExecPlan> {
  const path = isAbsolute(specPathArg) ? specPathArg : resolve(deps.repoRoot, specPathArg);
  const raw = await readFile(path, "utf8");
  const spec = parseSpecFile(raw, path);
  if (spec.frontmatter.mode !== "heavy") {
    throw new PlanRequiresHeavyModeError(spec.frontmatter.slug, spec.frontmatter.mode);
  }
  const title = extractTitle(spec.body) ?? spec.frontmatter.slug;
  const plan = await deps.planStore.create({
    slug: spec.frontmatter.slug,
    title,
    state: "specified",
    spec_path: path,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      plan_id: plan.id,
      from_state: null,
      to_state: "specified",
      trigger_verb: "plan:from-spec",
    },
  );
  return plan;
}

function extractTitle(body: string): string | undefined {
  const match = body.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : undefined;
}
