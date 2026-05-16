import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { dirExists, fileExists } from "@/shared/lib/fs.js";
import type {
  HandoffEmitterPort,
  HandoffEnvelope,
} from "./handoff-emitter.port.js";

const DEFAULT_DIR = ".maestro/handoffs";

export interface FsHandoffEmitterOptions {
  readonly repoRoot: string;
  readonly subdir?: string;
}

export class FsHandoffEmitter implements HandoffEmitterPort {
  readonly #dir: string;

  constructor(options: FsHandoffEmitterOptions) {
    this.#dir = join(options.repoRoot, options.subdir ?? DEFAULT_DIR);
  }

  async emit(envelope: HandoffEnvelope): Promise<void> {
    await mkdir(this.#dir, { recursive: true });
    await writeFile(
      this.#filePath(envelope.id),
      `${JSON.stringify(envelope, null, 2)}\n`,
      "utf8",
    );
  }

  async list(): Promise<readonly HandoffEnvelope[]> {
    if (!(await dirExists(this.#dir))) return [];
    const entries = await readdir(this.#dir);
    const files = entries.filter((e) => e.endsWith(".json"));
    const records: HandoffEnvelope[] = [];
    for (const f of files) {
      const raw = await readFile(join(this.#dir, f), "utf8");
      records.push(JSON.parse(raw) as HandoffEnvelope);
    }
    return records;
  }

  async get(id: string): Promise<HandoffEnvelope | undefined> {
    const path = this.#filePath(id);
    if (!(await fileExists(path))) return undefined;
    const raw = await readFile(path, "utf8");
    return JSON.parse(raw) as HandoffEnvelope;
  }

  #filePath(id: string): string {
    return join(this.#dir, `${id}.json`);
  }
}
