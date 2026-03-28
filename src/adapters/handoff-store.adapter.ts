import { join } from "node:path";
import type { Handoff, HandoffEnvelope, HandoffStatus } from "../domain/types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import { MaestroError } from "../domain/errors.js";
import { ensureDir, readJson, writeJson, listDirs } from "../lib/fs.js";

export class FsHandoffStoreAdapter implements HandoffStorePort {
  constructor(private readonly baseDir: string) {}

  private handoffDir(id: string): string {
    return join(this.baseDir, ".maestro", "handoffs", id);
  }

  private handoffPath(id: string): string {
    return join(this.handoffDir(id), "handoff.json");
  }

  private envelopePath(id: string): string {
    return join(this.handoffDir(id), "envelope.json");
  }

  async create(handoff: Handoff): Promise<string> {
    const dir = this.handoffDir(handoff.id);
    await ensureDir(dir);
    await writeJson(this.handoffPath(handoff.id), handoff);

    const envelope: HandoffEnvelope = {
      handoff,
      status: "pending",
    };
    await writeJson(this.envelopePath(handoff.id), envelope);
    return handoff.id;
  }

  async get(id: string): Promise<HandoffEnvelope | undefined> {
    return readJson<HandoffEnvelope>(this.envelopePath(id));
  }

  async getLatestPending(): Promise<HandoffEnvelope | undefined> {
    const all = await this.list({ status: "pending" });
    return all[0];
  }

  async list(
    filter?: { status?: HandoffStatus },
  ): Promise<readonly HandoffEnvelope[]> {
    const handoffsRoot = join(this.baseDir, ".maestro", "handoffs");
    const dirs = await listDirs(handoffsRoot);

    const envelopes: HandoffEnvelope[] = [];
    for (const dir of dirs) {
      const id = dir.split("/").pop()!;
      const envelope = await readJson<HandoffEnvelope>(this.envelopePath(id));
      if (!envelope) continue;
      if (filter?.status && envelope.status !== filter.status) continue;
      envelopes.push(envelope);
    }

    // Sort by timestamp descending (most recent first)
    envelopes.sort((a, b) =>
      b.handoff.timestamp.localeCompare(a.handoff.timestamp),
    );
    return envelopes;
  }

  async updateStatus(
    id: string,
    status: HandoffStatus,
    meta?: { pickedUpBy?: string; completedAt?: string; report?: string },
  ): Promise<void> {
    const envelope = await this.get(id);
    if (!envelope) {
      throw new MaestroError(`Handoff ${id} not found`, [
        "List handoffs: maestro handoff --list",
      ]);
    }

    const updated: HandoffEnvelope = {
      ...envelope,
      status,
      ...(meta?.pickedUpBy && {
        pickedUpBy: meta.pickedUpBy,
        pickedUpAt: new Date().toISOString(),
      }),
      ...(meta?.completedAt && { completedAt: meta.completedAt }),
      ...(meta?.report && { report: meta.report }),
    };

    await writeJson(this.envelopePath(id), updated);
  }
}
