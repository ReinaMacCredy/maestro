import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { ExecPlan, ExecPlanId } from "../types/exec-plan.js";
import { generateExecPlanId } from "../types/exec-plan.js";
import type { ExecPlanState } from "../types/exec-plan-state.js";
import {
  DuplicateExecPlanSlugError,
  ExecPlanNotFoundError,
  type CreateExecPlanInput,
  type ExecPlanPatch,
  type ExecPlanStorePort,
} from "./exec-plan-store.port.js";

const DEFAULT_PATH = ".maestro/plans/plans.v2.jsonl";

export interface JsonlExecPlanStoreOptions {
  readonly repoRoot: string;
  readonly file?: string;
  readonly clock?: () => Date;
  readonly idFactory?: () => ExecPlanId;
}

export class JsonlExecPlanStore implements ExecPlanStorePort {
  readonly #file: string;
  readonly #clock: () => Date;
  readonly #idFactory: () => ExecPlanId;
  #queue: Promise<void> = Promise.resolve();

  constructor(options: JsonlExecPlanStoreOptions) {
    this.#file = join(options.repoRoot, options.file ?? DEFAULT_PATH);
    this.#clock = options.clock ?? (() => new Date());
    this.#idFactory = options.idFactory ?? generateExecPlanId;
  }

  async create(input: CreateExecPlanInput): Promise<ExecPlan> {
    return this.#mutate(async (plans) => {
      if (plans.some((p) => p.slug === input.slug)) {
        throw new DuplicateExecPlanSlugError(input.slug);
      }
      const now = this.#clock().toISOString();
      const plan: ExecPlan = {
        id: this.#idFactory(),
        slug: input.slug,
        title: input.title,
        state: input.state,
        spec_path: input.spec_path,
        created_at: now,
        updated_at: now,
      };
      plans.push(plan);
      return plan;
    });
  }

  async get(id: ExecPlanId): Promise<ExecPlan | undefined> {
    const plans = await this.#read();
    return plans.find((p) => p.id === id);
  }

  async update(id: ExecPlanId, patch: ExecPlanPatch): Promise<ExecPlan> {
    return this.#mutate(async (plans) => {
      const idx = plans.findIndex((p) => p.id === id);
      if (idx === -1) throw new ExecPlanNotFoundError(id);
      const existing = plans[idx];
      if (!existing) throw new ExecPlanNotFoundError(id);
      const next: ExecPlan = {
        ...existing,
        ...patch,
        updated_at: this.#clock().toISOString(),
      };
      plans[idx] = next;
      return next;
    });
  }

  async list(): Promise<readonly ExecPlan[]> {
    return this.#read();
  }

  async listByState(state: ExecPlanState): Promise<readonly ExecPlan[]> {
    const plans = await this.#read();
    return plans.filter((p) => p.state === state);
  }

  #mutate<T>(fn: (plans: ExecPlan[]) => Promise<T>): Promise<T> {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason: unknown) => void;
    const result = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    this.#queue = this.#queue.then(async () => {
      try {
        const plans = [...(await this.#read())];
        const r = await fn(plans);
        await this.#write(plans);
        resolve(r);
      } catch (e) {
        reject(e);
      }
    });
    return result;
  }

  async #read(): Promise<readonly ExecPlan[]> {
    let text: string;
    try {
      text = await readFile(this.#file, "utf8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw err;
    }
    const out: ExecPlan[] = [];
    for (const line of text.split("\n")) {
      if (line.length === 0) continue;
      out.push(JSON.parse(line) as ExecPlan);
    }
    return out;
  }

  async #write(plans: readonly ExecPlan[]): Promise<void> {
    await mkdir(dirname(this.#file), { recursive: true });
    const body = plans.map((p) => JSON.stringify(p)).join("\n");
    await writeFile(this.#file, body.length > 0 ? `${body}\n` : "", "utf8");
  }
}
