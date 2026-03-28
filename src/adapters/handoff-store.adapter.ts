import { basename, join } from "node:path";
import type { Handoff, HandoffEnvelope, HandoffStatus } from "../domain/types.js";
import type { HandoffStorePort } from "../ports/handoff-store.port.js";
import { MAESTRO_DIR } from "../domain/defaults.js";
import { validateEnvelope, validateHandoff } from "../domain/validators.js";
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
    const validatedHandoff = validateHandoff(handoff);
    const dir = this.handoffDir(validatedHandoff.id);
    await ensureDir(dir);
    await writeJson(this.handoffPath(validatedHandoff.id), validatedHandoff);

    const envelope: HandoffEnvelope = {
      handoff: validatedHandoff,
      status: "pending",
    };
    const validatedEnvelope = validateEnvelope(envelope);
    await writeJson(this.envelopePath(validatedHandoff.id), validatedEnvelope);
    return validatedHandoff.id;
  }

  async get(id: string): Promise<HandoffEnvelope | undefined> {
    const envelope = await readJson<unknown>(this.envelopePath(id));
    if (!envelope) return undefined;
    return validateEnvelope(envelope);
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

    const settled = await Promise.allSettled(ids.map((id) => this.get(id)));
    const envelopes = settled
      .filter((r): r is PromiseFulfilledResult<HandoffEnvelope | undefined> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter(
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

    const validatedEnvelope = validateEnvelope(updated);
    await writeJson(this.envelopePath(id), validatedEnvelope);
    return validatedEnvelope;
  }
}
