import { tmpdir } from "node:os";
import { join } from "node:path";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { ConfigPort } from "@/infra/ports/config.port.js";
import type { ConfigLayers } from "@/infra/ports/config.port.js";
import type { SessionDetectPort } from "@/features/session";
import type { NotesStorePort } from "@/features/notes";
import type {
  MissionStorePort,
  FeatureStorePort,
  AssertionStorePort,
  CheckpointStorePort,
} from "@/features/mission";
import type {
  CorrectionStorePort,
  LearningStorePort,
  Correction,
  CreateCorrectionInput,
  CorrectionQuery,
  RawLearningEntry,
  CompiledLearnings,
} from "@/features/memory";
import type { RatchetStorePort, RatchetSuite, RatchetBaseline } from "@/features/ratchet";
import type { ProjectGraphStorePort, ProjectGraph } from "@/features/graph";
import type { HandoffLaunchRecord, LaunchStorePort } from "@/features/handoff";
import type { GitState } from "@/infra/domain/git-types.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import type { AgentSession } from "@/features/session";
import type { NoteEntry } from "@/features/notes";
import type {
  Mission,
  Feature,
  Assertion,
  Checkpoint,
  CreateMissionInput,
  CreateFeatureInput,
  CreateAssertionInput,
  UpdateMissionInput,
  UpdateFeatureInput,
  UpdateAssertionInput,
} from "@/features/mission";

export function mockGit(overrides: Partial<GitPort> = {}): GitPort {
  return {
    isRepo: async () => true,
    getState: async (): Promise<GitState> => ({
      branch: "main",
      recentCommits: ["abc1234 feat: test"],
      changedFiles: [],
      workingTreeClean: true,
      diffStat: "+0 -0",
    }),
    getCurrentBranch: async () => "main",
    createWorktree: async (_cwd, input) => ({
      slug: input.slug,
      baseBranch: input.baseBranch,
      branch: `${input.branchPrefix}/${input.slug}`,
      path: join(tmpdir(), input.slug),
    }),
    ...overrides,
  };
}

export function mockConfig(overrides: Partial<ConfigPort> = {}): ConfigPort {
  const store = new Map<string, MaestroConfig>();
  return {
    load: async () => ({ sessionDetection: { enabled: true, agents: ["claude-code"] } }),
    loadLayers: async (): Promise<ConfigLayers> => ({
      defaults: { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      effective: store.get("project") ?? store.get("global") ?? { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      project: store.get("project"),
      global: store.get("global"),
      errors: [],
      paths: {
        project: ".maestro/config.yaml",
        global: "~/.maestro/config.yaml",
      },
    }),
    write: async (scope, _dir, config) => {
      store.set(scope, config);
    },
    exists: async (scope) => store.has(scope),
    ...overrides,
  };
}

export function mockNotesStore(initial: NoteEntry[] = []): NotesStorePort {
  const notes = [...initial];

  return {
    append: async (note) => {
      notes.push(note);
    },
    list: async () => notes,
  };
}

export function mockSessionDetect(
  session?: AgentSession,
): SessionDetectPort {
  const defaultSession: AgentSession = {
    agent: "claude-code",
    sessionId: "test-session-123",
    sourcePath: join(tmpdir(), "sessions", "test"),
  };
  return {
    detect: async () => session ?? defaultSession,
    lookup: async (_agent, sessionId) => {
      const active = session ?? defaultSession;
      return active.sessionId === sessionId ? active : undefined;
    },
  };
}

// ============================
// Mission Control Mocks
// ============================

export function mockMissionStore(initial: Mission[] = []): MissionStorePort {
  const missions = new Map<string, Mission>();
  const staging = new Map<string, Mission>();

  for (const m of initial) {
    missions.set(m.id, m);
  }

  return {
    listIds: async () => [...missions.keys()].sort().reverse(),
    get: async (id: string) => missions.get(id) ?? staging.get(id),
    exists: async (id: string) => missions.has(id),
      stage: async (input: CreateMissionInput, id: string) => {
        const now = new Date().toISOString();
        const mission: Mission = {
          id,
          status: "draft",
          title: input.title,
          description: input.description,
          milestones: input.milestones.map((milestone) => ({
            ...milestone,
            featureIds: [],
          })),
          features: [],
          createdAt: now,
          updatedAt: now,
        };
      staging.set(id, mission);
      return id;
    },
    finalize: async (id: string) => {
      const staged = staging.get(id);
      if (staged) {
        missions.set(id, staged);
        staging.delete(id);
      }
    },
    update: async (id: string, input: UpdateMissionInput) => {
      const existing = missions.get(id);
      if (!existing) return undefined;

      const now = new Date().toISOString();
      const updated: Mission = {
        ...existing,
        ...(input.title !== undefined && { title: input.title }),
        ...(input.description !== undefined && { description: input.description }),
        ...(input.status !== undefined && { status: input.status }),
        updatedAt: now,
        ...(input.status === "approved" && { approvedAt: now }),
        ...(input.status === "rejected" && { rejectedAt: now }),
        ...(input.status === "completed" && { completedAt: now }),
      };
      missions.set(id, updated);
      return updated;
    },
    list: async () => {
      const all = [...missions.values()];
      all.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
      return all;
    },
  };
}

export function mockFeatureStore(
  missionId: string,
  initial: Feature[] = [],
): FeatureStorePort {
  const features = new Map<string, Feature>();

  for (const f of initial) {
    features.set(f.id, f);
  }

  return {
    get: async (_missionId: string, featureId: string) => features.get(featureId),
    exists: async (_missionId: string, featureId: string) => features.has(featureId),
      create: async (_missionId: string, input: CreateFeatureInput, id: string) => {
        const now = new Date().toISOString();
        const feature: Feature = {
          id,
          missionId,
        milestoneId: input.milestoneId,
        status: "pending",
          title: input.title,
          description: input.description,
          agentType: input.agentType,
          verificationSteps: input.verificationSteps,
          dependsOn: input.dependsOn ?? [],
          fulfills: input.fulfills ?? [],
          preconditions: input.preconditions,
          expectedBehavior: input.expectedBehavior,
          createdAt: now,
          updatedAt: now,
        };
      features.set(id, feature);
      return feature;
    },
    update: async (_missionId: string, featureId: string, input: UpdateFeatureInput) => {
      const existing = features.get(featureId);
      if (!existing) return undefined;

      const now = new Date().toISOString();
      const updated: Feature = {
        ...existing,
        ...(input.status !== undefined && { status: input.status }),
        ...(input.report !== undefined && { report: input.report }),
        updatedAt: now,
      };
      features.set(featureId, updated);
      return updated;
    },
    list: async (_missionId: string, filter?: { milestoneId?: string; status?: string }) => {
      let all = [...features.values()];
      if (filter?.milestoneId) {
        all = all.filter((f) => f.milestoneId === filter.milestoneId);
      }
      if (filter?.status) {
        all = all.filter((f) => f.status === filter.status);
      }
      all.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
      return all;
    },
    getMany: async (_missionId: string, featureIds: readonly string[]) => {
      return featureIds
        .map((id) => features.get(id))
        .filter((f): f is Feature => f !== undefined);
    },
  };
}

export function mockAssertionStore(
  missionId: string,
  initial: Assertion[] = [],
): AssertionStorePort {
  const assertions = new Map<string, Assertion>();

  for (const a of initial) {
    assertions.set(a.id, a);
  }

  return {
    get: async (_missionId: string, assertionId: string) => assertions.get(assertionId),
    exists: async (_missionId: string, assertionId: string) => assertions.has(assertionId),
      create: async (_missionId: string, input: CreateAssertionInput, id: string) => {
        const now = new Date().toISOString();
        const assertion: Assertion = {
          id,
          missionId,
        milestoneId: input.milestoneId,
          featureId: input.featureId,
          result: "pending",
          description: input.description,
          surface: input.surface ?? "cli",
          createdAt: now,
          updatedAt: now,
        };
      assertions.set(id, assertion);
      return assertion;
    },
    update: async (_missionId: string, assertionId: string, input: UpdateAssertionInput) => {
      const existing = assertions.get(assertionId);
      if (!existing) return undefined;

      const now = new Date().toISOString();
      const updated: Assertion = {
        id: existing.id,
        missionId: existing.missionId,
          milestoneId: existing.milestoneId,
          featureId: existing.featureId,
          description: existing.description,
          surface: existing.surface,
          createdAt: existing.createdAt,
          result: input.result,
          updatedAt: now,
        evidence: input.evidence,
        waivedReason: input.waivedReason,
      };
      assertions.set(assertionId, updated);
      return updated;
    },
    list: async (_missionId: string) => {
      const all = [...assertions.values()];
      all.sort((a, b) => a.createdAt.localeCompare(b.createdAt));
      return all;
    },
    listByMilestone: async (_missionId: string, milestoneId: string) => {
      return [...assertions.values()].filter((a) => a.milestoneId === milestoneId);
    },
    getMany: async (_missionId: string, assertionIds: readonly string[]) => {
      return assertionIds
        .map((id) => assertions.get(id))
        .filter((a): a is Assertion => a !== undefined);
    },
  };
}

export function mockCheckpointStore(
  _missionId: string,
  initial: Checkpoint[] = [],
): CheckpointStorePort {
  const checkpoints = new Map<string, Checkpoint>();
  let counter = 0;

  for (const c of initial) {
    checkpoints.set(c.id, c);
  }

  return {
    get: async (__missionId: string, checkpointId: string) => checkpoints.get(checkpointId),
    save: async (__missionId: string, data: Omit<Checkpoint, "id">) => {
      counter++;
      const id = `checkpoint-${counter}`;
      const checkpoint: Checkpoint = {
        id,
        missionId: data.missionId,
        currentMilestoneId: data.currentMilestoneId,
        timestamp: data.timestamp,
        featureStatuses: data.featureStatuses,
        assertionResults: data.assertionResults,
      };
      checkpoints.set(id, checkpoint);
      return checkpoint;
    },
    list: async (_missionId: string) => {
      const all = [...checkpoints.values()];
      all.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      return all;
    },
    getLatest: async (_missionId: string) => {
      const all = [...checkpoints.values()];
      all.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      return all[0];
    },
    load: async (_mId: string) => {
      const all = [...checkpoints.values()];
      all.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      return all[0];
    },
    };
  }

export function mockCorrectionStore(
  initial: Correction[] = [],
): CorrectionStorePort {
  const corrections = new Map<string, Correction>();
  let counter = initial.length;

  for (const c of initial) {
    corrections.set(c.id, c);
  }

  return {
    create: async (input: CreateCorrectionInput) => {
      counter++;
      const now = new Date().toISOString();
      const id = `corr-${counter}`;
      const correction: Correction = {
        id,
        rule: input.rule,
        source: input.source,
        trigger: input.trigger,
        severity: input.severity,
        createdAt: now,
        updatedAt: now,
      };
      corrections.set(id, correction);
      return correction;
    },
    get: async (id: string) => corrections.get(id),
    list: async () => [...corrections.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    search: async (query: CorrectionQuery) => {
      const all = [...corrections.values()];
      return all.filter((c) => {
        if (query.keywords?.length) {
          const match = query.keywords.some((qk) =>
            c.trigger.keywords.some((tk) => tk.toLowerCase().includes(qk.toLowerCase())),
          );
          if (!match) return false;
        }
        if (query.text) {
          const text = query.text.toLowerCase();
          if (!c.rule.toLowerCase().includes(text) && !c.source.toLowerCase().includes(text)) return false;
        }
        return true;
      });
    },
    update: async (id: string, input: Partial<Correction>) => {
      const existing = corrections.get(id);
      if (!existing) return undefined;
      const updated: Correction = { ...existing, ...input, id: existing.id, updatedAt: new Date().toISOString() };
      corrections.set(id, updated);
      return updated;
    },
    remove: async (id: string) => corrections.delete(id),
  };
}

export function mockLearningStore(
  initial: RawLearningEntry[] = [],
): LearningStorePort {
  const raw = [...initial];
  let compiled: CompiledLearnings | undefined;

  return {
    appendRaw: async (entry: RawLearningEntry) => {
      raw.push(entry);
    },
    listRaw: async () => [...raw],
    rawCount: async () => raw.length,
    readCompiled: async () => compiled,
    writeCompiled: async (c: CompiledLearnings) => {
      compiled = c;
    },
  };
}

export function mockRatchetStore(
  initialSuite?: RatchetSuite,
  initialBaseline?: RatchetBaseline,
): RatchetStorePort {
  let suite: RatchetSuite = initialSuite ?? { assertions: [] };
  let baseline: RatchetBaseline | undefined = initialBaseline;

  return {
    getSuite: async () => suite,
    writeSuite: async (s: RatchetSuite) => {
      suite = s;
    },
    getBaseline: async () => baseline,
    writeBaseline: async (b: RatchetBaseline) => {
      baseline = b;
    },
  };
}

export function mockProjectGraphStore(
  initial?: ProjectGraph,
): ProjectGraphStorePort {
  let graph: ProjectGraph = initial ?? { nodes: [], edges: [] };

  return {
    load: async () => graph,
    save: async (g: ProjectGraph) => {
      graph = g;
    },
  };
}

export function makeHandoffLaunchRecord(
  partial: Partial<HandoffLaunchRecord> & { id: string; createdAt: string },
): HandoffLaunchRecord {
  return {
    id: partial.id,
    createdAt: partial.createdAt,
    task: partial.task ?? "work",
    name: partial.name ?? partial.id,
    agent: partial.agent ?? "codex",
    model: partial.model ?? "gpt-5.4",
    status: partial.status ?? "launched",
    wait: partial.wait ?? false,
    sourceDir: partial.sourceDir ?? "/src",
    targetDir: partial.targetDir ?? "/target",
    promptPath: partial.promptPath ?? "prompt.md",
    outputPath: partial.outputPath ?? "output.log",
    command: partial.command ?? [],
    refs: partial.refs ?? {},
    ...(partial.consumedAt !== undefined ? { consumedAt: partial.consumedAt } : {}),
    ...(partial.pickedUpByAgent !== undefined ? { pickedUpByAgent: partial.pickedUpByAgent } : {}),
    ...(partial.pickedUpBySessionId !== undefined ? { pickedUpBySessionId: partial.pickedUpBySessionId } : {}),
    ...(partial.pickedUpAt !== undefined ? { pickedUpAt: partial.pickedUpAt } : {}),
    ...(partial.worktree !== undefined ? { worktree: partial.worktree } : {}),
  };
}

export function mockLaunchStore(records: readonly HandoffLaunchRecord[] = []): LaunchStorePort {
  return {
    async create() { throw new Error("not used in mockLaunchStore"); },
    async update(r) { return r; },
    async consume() { throw new Error("not used in mockLaunchStore"); },
    async get(id) { return records.find((r) => r.id === id); },
    async list() { return records; },
    resolveArtifactPath(p: string) { return p; },
  };
}
