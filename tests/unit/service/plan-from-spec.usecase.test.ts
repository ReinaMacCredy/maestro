import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type {
  CreateExecPlanInput,
  ExecPlanPatch,
  ExecPlanStorePort,
} from "@/repo/exec-plan-store.port.js";
import { DuplicateExecPlanSlugError } from "@/repo/exec-plan-store.port.js";
import type {
  EvidenceFilter,
  EvidenceRow,
  EvidenceStorePort,
} from "@/repo/evidence-store.port.js";
import type { ExecPlan, ExecPlanId } from "@/types/exec-plan.js";
import type { ExecPlanState } from "@/types/exec-plan-state.js";
import {
  planFromSpec,
  PlanRequiresHeavyModeError,
} from "@/service/plan-from-spec.usecase.js";

const FROZEN = new Date("2026-05-15T11:00:00.000Z");

function makeStores(): {
  planStore: ExecPlanStorePort;
  plans: Map<ExecPlanId, ExecPlan>;
  evidenceStore: EvidenceStorePort;
  evidence: EvidenceRow[];
} {
  const plans = new Map<ExecPlanId, ExecPlan>();
  const evidence: EvidenceRow[] = [];
  let n = 0;
  const planStore: ExecPlanStorePort = {
    async create(input: CreateExecPlanInput) {
      n += 1;
      const plan: ExecPlan = {
        id: `pln-${n}`,
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        created_at: FROZEN.toISOString(),
        updated_at: FROZEN.toISOString(),
      };
      if ([...plans.values()].some((p) => p.slug === input.slug)) {
        throw new DuplicateExecPlanSlugError(input.slug);
      }
      plans.set(plan.id, plan);
      return plan;
    },
    async get(id) {
      return plans.get(id);
    },
    async update(id, patch: ExecPlanPatch) {
      const existing = plans.get(id);
      if (!existing) throw new Error("not found");
      const next: ExecPlan = { ...existing, ...patch, updated_at: FROZEN.toISOString() };
      plans.set(id, next);
      return next;
    },
    async list() {
      return [...plans.values()];
    },
    async listByState(state: ExecPlanState) {
      return [...plans.values()].filter((p) => p.state === state);
    },
  };
  const evidenceStore: EvidenceStorePort = {
    async append(row) {
      evidence.push(row);
    },
    async list(_filter?: EvidenceFilter) {
      return evidence;
    },
  };
  return { planStore, plans, evidenceStore, evidence };
}

const HEAVY_SPEC = `---
slug: demo-plan
acceptance_criteria:
  - it works
non_goals:
  - nothing
risk_class: medium
mode: heavy
work_type: change-request
---

# Demo plan

A heavy-mode spec.
`;

const LIGHT_SPEC = `---
slug: demo-light
acceptance_criteria:
  - it works
non_goals:
  - nothing
risk_class: low
mode: light
work_type: change-request
---

# Demo light

A light-mode spec.
`;

describe("planFromSpec", () => {
  let repoRoot: string;

  beforeEach(async () => {
    repoRoot = await mkdtemp(join(tmpdir(), "v2-plan-from-spec-"));
    await mkdir(join(repoRoot, ".maestro/specs"), { recursive: true });
  });

  afterEach(async () => {
    await rm(repoRoot, { recursive: true, force: true });
  });

  it("creates an ExecPlan in 'specified' from a heavy-mode spec and emits one transition row", async () => {
    const specPath = join(repoRoot, ".maestro/specs/demo-plan.md");
    await writeFile(specPath, HEAVY_SPEC);
    const { planStore, evidenceStore, evidence } = makeStores();

    const plan = await planFromSpec(
      { repoRoot, planStore, evidenceStore },
      ".maestro/specs/demo-plan.md",
    );

    expect(plan.id).toBe("pln-1");
    expect(plan.slug).toBe("demo-plan");
    expect(plan.state).toBe("specified");
    expect(plan.title).toBe("Demo plan");
    expect(plan.spec_path).toBe(specPath);

    expect(evidence.length).toBe(1);
    expect(evidence[0]).toMatchObject({
      kind: "transition",
      plan_id: "pln-1",
      from_state: null,
      to_state: "specified",
      trigger_verb: "plan:from-spec",
    });
  });

  it("throws PlanRequiresHeavyModeError for a light-mode spec", async () => {
    const specPath = join(repoRoot, ".maestro/specs/demo-light.md");
    await writeFile(specPath, LIGHT_SPEC);
    const { planStore, evidenceStore, evidence } = makeStores();

    await expect(
      planFromSpec({ repoRoot, planStore, evidenceStore }, ".maestro/specs/demo-light.md"),
    ).rejects.toBeInstanceOf(PlanRequiresHeavyModeError);

    expect(evidence.length).toBe(0);
  });

  it("rejects duplicate slug via the underlying store", async () => {
    const specPath = join(repoRoot, ".maestro/specs/demo-plan.md");
    await writeFile(specPath, HEAVY_SPEC);
    const { planStore, evidenceStore } = makeStores();

    await planFromSpec({ repoRoot, planStore, evidenceStore }, ".maestro/specs/demo-plan.md");
    await expect(
      planFromSpec({ repoRoot, planStore, evidenceStore }, ".maestro/specs/demo-plan.md"),
    ).rejects.toBeInstanceOf(DuplicateExecPlanSlugError);
  });
});
