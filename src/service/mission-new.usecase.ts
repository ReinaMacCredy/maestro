import { isAbsolute, resolve } from "node:path";
import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { MissionStorePort } from "../repo/mission-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { readJson } from "@/shared/lib/fs.js";
import { generateSpecSlug } from "../types/spec-id.js";
import type { Mission } from "../types/mission.js";
import type { Task } from "../types/task.js";
import {
  missionDecompose,
  parseMissionDecomposeBatch,
  type MissionDecomposeTaskInput,
} from "./mission-decompose.usecase.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";
import { missionFromSpec } from "./mission-from-spec.usecase.js";
import { loadTemplate } from "../features/mission/templates/loader.js";

export type MissionNewMode = "bare" | "from-spec" | "from-file" | "template";

export interface MissionNewInput {
  readonly title: string;
  readonly slug: string;
  readonly mode: MissionNewMode;
  readonly fromSpec?: string;
  readonly fromFile?: string;
  readonly template?: string;
}

export interface MissionNewDeps {
  readonly repoRoot: string;
  readonly missionStore: MissionStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export interface MissionNewResult {
  readonly mission: Mission;
  readonly tasks: readonly Task[];
}

export class MissionNewInvalidFlagsError extends Error {
  constructor(message: string) {
    super(`mission new: ${message}`);
    this.name = "MissionNewInvalidFlagsError";
  }
}

export class MissionTemplateUnknownError extends Error {
  readonly templateName: string;
  constructor(templateName: string) {
    super(`mission new: unknown template '${templateName}'`);
    this.name = "MissionTemplateUnknownError";
    this.templateName = templateName;
  }
}

export async function missionNew(
  deps: MissionNewDeps,
  input: MissionNewInput,
): Promise<MissionNewResult> {
  if (input.mode === "from-spec") {
    if (!input.fromSpec) throw new MissionNewInvalidFlagsError("--from-spec requires a path");
    const mission = await missionFromSpec(
      {
        repoRoot: deps.repoRoot,
        missionStore: deps.missionStore,
        evidenceStore: deps.evidenceStore,
        clock: deps.clock,
        idFactory: deps.idFactory,
      },
      input.fromSpec,
    );
    return { mission, tasks: [] };
  }

  if (input.mode === "from-file") {
    if (!input.fromFile) throw new MissionNewInvalidFlagsError("--from-file requires a path");
    const tasks = await readDecomposeBatch(deps.repoRoot, input.fromFile);
    return seedIntakeAndDecompose(deps, input.slug, input.title, tasks);
  }

  if (input.mode === "template") {
    if (!input.template) throw new MissionNewInvalidFlagsError("--template requires a name");
    const template = await loadTemplate(input.template, deps.repoRoot);
    if (!template) throw new MissionTemplateUnknownError(input.template);
    const tasks: MissionDecomposeTaskInput[] = template.seedTasks.map((t) => ({
      title: t.title,
      slug: `${input.slug}-${t.slug}`,
    }));
    return seedIntakeAndDecompose(deps, input.slug, input.title, tasks);
  }

  const mission = await createIntakeMission(deps, input.slug, input.title);
  return { mission, tasks: [] };
}

async function seedIntakeAndDecompose(
  deps: MissionNewDeps,
  slug: string,
  title: string,
  tasks: readonly MissionDecomposeTaskInput[],
): Promise<MissionNewResult> {
  const mission = await createIntakeMission(deps, slug, title);
  return missionDecompose(
    {
      missionStore: deps.missionStore,
      taskStore: deps.taskStore,
      evidenceStore: deps.evidenceStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    { mission_id: mission.id, tasks },
  );
}

async function createIntakeMission(
  deps: MissionNewDeps,
  slug: string,
  title: string,
): Promise<Mission> {
  const mission = await deps.missionStore.create({
    slug,
    title,
    state: "intake",
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
      to_state: "intake",
      trigger_verb: "mission:new",
      trigger: "verb",
    },
  );
  return mission;
}

async function readDecomposeBatch(
  repoRoot: string,
  filePath: string,
): Promise<readonly MissionDecomposeTaskInput[]> {
  const path = isAbsolute(filePath) ? filePath : resolve(repoRoot, filePath);
  let parsed: unknown;
  try {
    parsed = await readJson(path);
  } catch (err) {
    throw new MissionNewInvalidFlagsError(
      `--from-file ${filePath}: invalid JSON: ${(err as Error).message}`,
    );
  }
  if (parsed === undefined) {
    throw new MissionNewInvalidFlagsError(`--from-file ${filePath}: file not found`);
  }
  return parseMissionDecomposeBatch(parsed);
}

export function slugifyTitle(title: string): string {
  const slug = generateSpecSlug(title);
  if (slug.length === 0) {
    throw new MissionNewInvalidFlagsError(
      `title '${title}' slugs to empty; pass --slug explicitly`,
    );
  }
  return slug;
}
