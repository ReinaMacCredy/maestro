import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { Mission, MissionId } from "../types/mission.js";
import { generateMissionId } from "../types/mission.js";
import type { MissionState } from "../types/mission-state.js";
import {
  DuplicateMissionSlugError,
  MissionNotFoundError,
  type CreateMissionInput,
  type MissionPatch,
  type MissionStorePort,
} from "./mission-store.port.js";

const DEFAULT_PATH = ".maestro/missions/plans.jsonl";

export interface JsonlMissionStoreOptions {
  readonly repoRoot: string;
  readonly file?: string;
  readonly clock?: () => Date;
  readonly idFactory?: () => MissionId;
}

export class JsonlMissionStore implements MissionStorePort {
  readonly #file: string;
  readonly #clock: () => Date;
  readonly #idFactory: () => MissionId;
  #queue: Promise<void> = Promise.resolve();

  constructor(options: JsonlMissionStoreOptions) {
    this.#file = join(options.repoRoot, options.file ?? DEFAULT_PATH);
    this.#clock = options.clock ?? (() => new Date());
    this.#idFactory = options.idFactory ?? generateMissionId;
  }

  async create(input: CreateMissionInput): Promise<Mission> {
    return this.#mutate(async (plans) => {
      if (plans.some((p) => p.slug === input.slug)) {
        throw new DuplicateMissionSlugError(input.slug);
      }
      const now = this.#clock().toISOString();
      const plan: Mission = {
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

  async get(id: MissionId): Promise<Mission | undefined> {
    const plans = await this.#read();
    return plans.find((p) => p.id === id);
  }

  async update(id: MissionId, patch: MissionPatch): Promise<Mission> {
    return this.#mutate(async (plans) => {
      const idx = plans.findIndex((p) => p.id === id);
      if (idx === -1) throw new MissionNotFoundError(id);
      const existing = plans[idx];
      if (!existing) throw new MissionNotFoundError(id);
      const next: Mission = {
        ...existing,
        ...patch,
        updated_at: this.#clock().toISOString(),
      };
      plans[idx] = next;
      return next;
    });
  }

  async list(): Promise<readonly Mission[]> {
    return this.#read();
  }

  async listByState(state: MissionState): Promise<readonly Mission[]> {
    const plans = await this.#read();
    return plans.filter((p) => p.state === state);
  }

  #mutate<T>(fn: (plans: Mission[]) => Promise<T>): Promise<T> {
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

  async #read(): Promise<readonly Mission[]> {
    let text: string;
    try {
      text = await readFile(this.#file, "utf8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw err;
    }
    const out: Mission[] = [];
    for (const line of text.split("\n")) {
      if (line.length === 0) continue;
      out.push(JSON.parse(line) as Mission);
    }
    return out;
  }

  async #write(plans: readonly Mission[]): Promise<void> {
    await mkdir(dirname(this.#file), { recursive: true });
    const body = plans.map((p) => JSON.stringify(p)).join("\n");
    await writeFile(this.#file, body.length > 0 ? `${body}\n` : "", "utf8");
  }
}
