import type {
  HandoffAgent,
  HandoffLaunchRecord,
  HandoffRefs,
  HandoffWorktree,
  LaunchStorePort,
} from "../domain/launch-types.js";

export class CompositeLaunchStore implements LaunchStorePort {
  constructor(
    private readonly local: LaunchStorePort,
    private readonly global: LaunchStorePort,
  ) {}

  async create(input: {
    readonly task: string;
    readonly name: string;
    readonly agent: HandoffAgent;
    readonly model: string;
    readonly wait: boolean;
    readonly sourceDir: string;
    readonly targetDir: string;
    readonly refs: HandoffRefs;
    readonly createdByAgent?: string;
    readonly createdBySessionId?: string;
    readonly worktree?: HandoffWorktree;
    readonly prompt: string;
  }): Promise<HandoffLaunchRecord> {
    const store = input.refs.taskId ? this.local : this.global;
    return store.create(input);
  }

  async update(record: HandoffLaunchRecord): Promise<HandoffLaunchRecord> {
    const store = record.refs.taskId ? this.local : this.global;
    return store.update(record);
  }

  async consume(input: {
    readonly id: string;
    readonly agent: string;
    readonly sessionId?: string;
    readonly pickedUpAt: string;
  }): Promise<HandoffLaunchRecord> {
    const localRecord = await this.local.get(input.id);
    if (localRecord) {
      return this.local.consume(input);
    }
    return this.global.consume(input);
  }

  async get(id: string): Promise<HandoffLaunchRecord | undefined> {
    return (await this.local.get(id)) ?? (await this.global.get(id));
  }

  async list(): Promise<readonly HandoffLaunchRecord[]> {
    const [localRecords, globalRecords] = await Promise.all([this.local.list(), this.global.list()]);
    const seen = new Set<string>();
    const merged: HandoffLaunchRecord[] = [];
    for (const record of [...localRecords, ...globalRecords]) {
      if (seen.has(record.id)) continue;
      seen.add(record.id);
      merged.push(record);
    }
    return merged;
  }

  resolveArtifactPath(relativePath: string, refs: HandoffRefs): string {
    const store = refs.taskId ? this.local : this.global;
    return store.resolveArtifactPath(relativePath, refs);
  }
}
