import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import { MissionNotFoundError } from "../repo/mission-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { DuplicateSlugError } from "../repo/task-store.port.js";
import type { Mission, MissionId } from "../types/mission.js";
import { assertMissionTransition } from "../types/mission-state.js";
import type { Task } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface MissionDecomposeTaskInput {
  readonly title: string;
  readonly slug: string;
  readonly spec_path?: string;
}

export interface MissionDecomposeDeps {
  readonly missionStore: MissionStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface MissionDecomposeInput {
  readonly mission_id: MissionId;
  readonly tasks: readonly MissionDecomposeTaskInput[];
}

export interface MissionDecomposeResult {
  readonly mission: Mission;
  readonly tasks: readonly Task[];
}

export class MissionDecomposeBatchEmptyError extends Error {
  constructor() {
    super("mission decompose requires at least one task in the batch");
    this.name = "MissionDecomposeBatchEmptyError";
  }
}

export class MissionDecomposeBatchInvalidError extends Error {
  readonly index: number;
  readonly field: string;
  constructor(index: number, field: string, detail: string) {
    super(`mission decompose: task[${index}].${field} ${detail}`);
    this.name = "MissionDecomposeBatchInvalidError";
    this.index = index;
    this.field = field;
  }
}

export class MissionDecomposeDuplicateSlugInBatchError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`mission decompose: slug '${slug}' appears more than once in the batch`);
    this.name = "MissionDecomposeDuplicateSlugInBatchError";
    this.slug = slug;
  }
}

export class MissionDecomposeAlreadyHasTasksError extends Error {
  readonly missionId: string;
  readonly count: number;
  constructor(missionId: string, count: number) {
    super(
      `mission ${missionId} already has ${count} task(s); decompose only accepts missions with zero tasks. Use \`task new\` to add more tasks manually, or \`mission cancel\` and start over.`,
    );
    this.name = "MissionDecomposeAlreadyHasTasksError";
    this.missionId = missionId;
    this.count = count;
  }
}

export function parseMissionDecomposeBatch(raw: unknown): readonly MissionDecomposeTaskInput[] {
  const arr = Array.isArray(raw)
    ? raw
    : raw !== null &&
        typeof raw === "object" &&
        Array.isArray((raw as { tasks?: unknown }).tasks)
      ? (raw as { tasks: unknown[] }).tasks
      : undefined;
  if (!arr) {
    throw new MissionDecomposeBatchInvalidError(
      -1,
      "root",
      "must be a JSON array, or an object with a 'tasks' array",
    );
  }
  if (arr.length === 0) throw new MissionDecomposeBatchEmptyError();
  const tasks: MissionDecomposeTaskInput[] = [];
  for (let i = 0; i < arr.length; i += 1) {
    const t = arr[i];
    if (t === null || typeof t !== "object") {
      throw new MissionDecomposeBatchInvalidError(i, "self", "must be an object");
    }
    const rec = t as Record<string, unknown>;
    if (typeof rec.title !== "string" || rec.title.trim().length === 0) {
      throw new MissionDecomposeBatchInvalidError(i, "title", "must be a non-empty string");
    }
    if (typeof rec.slug !== "string" || rec.slug.trim().length === 0) {
      throw new MissionDecomposeBatchInvalidError(i, "slug", "must be a non-empty string");
    }
    if (
      rec.spec_path !== undefined &&
      (typeof rec.spec_path !== "string" || rec.spec_path.length === 0)
    ) {
      throw new MissionDecomposeBatchInvalidError(i, "spec_path", "must be a non-empty string if set");
    }
    tasks.push({
      title: rec.title,
      slug: rec.slug,
      spec_path: rec.spec_path as string | undefined,
    });
  }
  return tasks;
}

export async function missionDecompose(
  deps: MissionDecomposeDeps,
  input: MissionDecomposeInput,
): Promise<MissionDecomposeResult> {
  if (input.tasks.length === 0) throw new MissionDecomposeBatchEmptyError();
  const seen = new Set<string>();
  for (const t of input.tasks) {
    if (seen.has(t.slug)) throw new MissionDecomposeDuplicateSlugInBatchError(t.slug);
    seen.add(t.slug);
  }

  const mission = await deps.missionStore.get(input.mission_id);
  if (!mission) throw new MissionNotFoundError(input.mission_id);
  assertMissionTransition(mission.state, "planned");

  const existingForMission = await deps.taskStore.listByMissionId(mission.id);
  if (existingForMission.length > 0) {
    throw new MissionDecomposeAlreadyHasTasksError(mission.id, existingForMission.length);
  }

  const existingSlugs = new Set((await deps.taskStore.list()).map((e) => e.slug));
  for (const t of input.tasks) {
    if (existingSlugs.has(t.slug)) throw new DuplicateSlugError(t.slug);
  }

  const created: Task[] = [];
  for (const t of input.tasks) {
    const task = await deps.taskStore.create({
      slug: t.slug,
      title: t.title,
      state: "draft",
      spec_path: t.spec_path,
      mission_id: mission.id,
    });
    created.push(task);
    await emitTransitionEvidence(
      {
        store: deps.evidenceStore,
        observabilityStore: deps.observabilityStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      {
        task_id: task.id,
        mission_id: mission.id,
        from_state: null,
        to_state: "draft",
        trigger_verb: "task:from-spec",
      },
    );
  }

  const updatedMission = await deps.missionStore.update(mission.id, { state: "planned" });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      mission_id: mission.id,
      from_state: mission.state,
      to_state: "planned",
      trigger_verb: "mission:decompose",
    },
  );

  return { mission: updatedMission, tasks: created };
}
