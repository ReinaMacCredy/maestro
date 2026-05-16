import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { MaestroError } from "@/shared/errors.js";
import { dirExists, fileExists } from "@/shared/lib/fs.js";
import type {
  HandoffEmitterPort,
  HandoffEnvelope,
  HandoffPickup,
} from "./handoff-emitter.port.js";

const DEFAULT_DIR = ".maestro/handoffs";
const PICKUP_SUFFIX = ".picked_up.json";

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
    const files = entries.filter(
      (e) => e.endsWith(".json") && !e.endsWith(PICKUP_SUFFIX),
    );
    const records: HandoffEnvelope[] = [];
    for (const f of files) {
      const raw = await readFile(join(this.#dir, f), "utf8");
      try {
        records.push(JSON.parse(raw) as HandoffEnvelope);
      } catch {
        // Skip malformed envelopes so one corrupt file doesn't take down list().
        console.warn(`handoff list: skipping malformed envelope ${f}`);
      }
    }
    return records;
  }

  async get(id: string): Promise<HandoffEnvelope | undefined> {
    const path = this.#filePath(id);
    if (!(await fileExists(path))) return undefined;
    const raw = await readFile(path, "utf8");
    try {
      return JSON.parse(raw) as HandoffEnvelope;
    } catch {
      throw new MaestroError(
        `Handoff envelope ${id} is malformed JSON`,
        [
          `Inspect the file on disk: .maestro/handoffs/${id}.json`,
          "Delete or repair the file, then retry",
        ],
        "HANDOFF_MALFORMED",
      );
    }
  }

  async markPickedUp(envelopeId: string, pickup: HandoffPickup): Promise<void> {
    await mkdir(this.#dir, { recursive: true });
    const path = this.#pickupPath(envelopeId);
    try {
      await writeFile(path, `${JSON.stringify(pickup, null, 2)}\n`, {
        encoding: "utf8",
        flag: "wx",
      });
    } catch (err) {
      if (isEexist(err)) {
        throw new MaestroError(
          `Handoff ${envelopeId} already picked up`,
          [
            "Read the existing pickup via maestro_handoff_show",
            "Choose a different envelope to pick up",
          ],
          "HANDOFF_ALREADY_PICKED_UP",
        );
      }
      throw err;
    }
  }

  async getPickup(envelopeId: string): Promise<HandoffPickup | undefined> {
    const path = this.#pickupPath(envelopeId);
    if (!(await fileExists(path))) return undefined;
    const raw = await readFile(path, "utf8");
    return JSON.parse(raw) as HandoffPickup;
  }

  #filePath(id: string): string {
    return join(this.#dir, `${id}.json`);
  }

  #pickupPath(id: string): string {
    return join(this.#dir, `${id}${PICKUP_SUFFIX}`);
  }
}

function isEexist(err: unknown): boolean {
  return (
    typeof err === "object" &&
    err !== null &&
    (err as { code?: string }).code === "EEXIST"
  );
}
