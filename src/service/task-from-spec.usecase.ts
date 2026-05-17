import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import type { EvidenceStorePort } from "../repo/evidence-store.port.js";
import type { ObservabilityPort } from "../repo/observability.port.js";
import type { SpecStorePort } from "../repo/spec-store.port.js";
import type { TaskStorePort } from "../repo/task-store.port.js";
import { parseSpecFile } from "../repo/fs-spec-store.adapter.js";
import type { Task } from "../types/task.js";
import { emitTransitionEvidence } from "./emit-transition-evidence.js";

export interface TaskFromSpecDeps {
  readonly repoRoot: string;
  readonly specStore: SpecStorePort;
  readonly taskStore: TaskStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly observabilityStore?: ObservabilityPort;
  readonly clock?: () => Date;
  readonly idFactory?: () => string;
}

export class SpecFileNotFoundError extends Error {
  readonly path: string;
  readonly inputArg: string;
  constructor(path: string, inputArg: string) {
    super(`Spec file not found: ${path}`);
    this.name = "SpecFileNotFoundError";
    this.path = path;
    this.inputArg = inputArg;
  }
}

export async function taskFromSpec(
  deps: TaskFromSpecDeps,
  specPathArg: string,
): Promise<Task> {
  const path = isAbsolute(specPathArg) ? specPathArg : resolve(deps.repoRoot, specPathArg);
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      throw new SpecFileNotFoundError(path, specPathArg);
    }
    throw err;
  }
  const spec = parseSpecFile(raw, path);
  const title = extractTitle(spec.body) ?? spec.frontmatter.slug;
  const task = await deps.taskStore.create({
    slug: spec.frontmatter.slug,
    title,
    state: "draft",
    spec_path: path,
  });
  await emitTransitionEvidence(
    {
      store: deps.evidenceStore,
      observabilityStore: deps.observabilityStore,
      clock: deps.clock,
      idFactory: deps.idFactory,
    },
    {
      task_id: task.id,
      from_state: null,
      to_state: "draft",
      trigger_verb: "task:from-spec",
    },
  );
  return task;
}

function extractTitle(body: string): string | undefined {
  const match = body.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : undefined;
}
