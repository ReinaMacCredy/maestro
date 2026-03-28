import { basename, join } from "node:path";
import type { Handoff, HandoffEnvelope, HandoffStatus } from "../domain/types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { ensureDir, readJson, writeJson, listDirs } from "../lib/fs.js";

export class FsHandoffStoreAdapter implements HandoffStorePort {
  constructor(private readonly baseDir: string) {}

  private handoffsRoot(): string {
    return join(this.baseDir, MAESTRO_DIR, "handoffs");
  }

  private handoffDir(id: string): string {
    return join(this.handoffsRoot(), id);
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

  async listIds(): Promise<readonly string[]> {
    const dirs = await listDirs(this.handoffsRoot());
    return dirs.map((d) => basename(d)).sort().reverse();
  }

  async list(
    filter?: { status?: HandoffStatus },
  ): Promise<readonly HandoffEnvelope[]> {
    const dirs = await listDirs(this.handoffsRoot());
    const ids = dirs.map((d) => basename(d));

    const results = await Promise.all(ids.map((id) => this.get(id)));
    const envelopes = results.filter(
      (e): e is HandoffEnvelope =>
        e !== undefined && (!filter?.status || e.status === filter.status),
    );

    envelopes.sort((a, b) =>
      b.handoff.timestamp.localeCompare(a.handoff.timestamp),
    );
    return envelopes;
  }

  async updateStatus(
    id: string,
    status: HandoffStatus,
    meta?: { pickedUpBy?: string; completedAt?: string; report?: string },
  ): Promise<HandoffEnvelope | undefined> {
    const envelope = await this.get(id);
    if (!envelope) return undefined;

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
    return updated;
  }
}
