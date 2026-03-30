import type { GitPort } from "../../src/ports/git.port.js";
import type { ConfigPort } from "../../src/ports/config.port.js";
import type { HandoffStorePort } from "../../src/ports/handoff-store.port.js";
import type { CassPort } from "../../src/ports/cass.port.js";
import type { SessionDetectPort } from "../../src/ports/session-detect.port.js";
import type { NotesStorePort } from "../../src/ports/notes-store.port.js";
import type { MissionStorePort } from "../../src/ports/mission-store.port.js";
import type { FeatureStorePort } from "../../src/ports/feature-store.port.js";
import type { AssertionStorePort } from "../../src/ports/assertion-store.port.js";
import type { CheckpointStorePort } from "../../src/ports/checkpoint-store.port.js";
import type {
  GitState,
  MaestroConfig,
  Handoff,
  HandoffEnvelope,
  HandoffStatus,
  CassSearchResponse,
  HandoffSession,
  NoteEntry,
} from "../../src/domain/types.js";
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
} from "../../src/domain/mission-types.js";

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
    ...overrides,
  };
}

export function mockConfig(overrides: Partial<ConfigPort> = {}): ConfigPort {
  const store = new Map<string, MaestroConfig>();
  return {
    load: async () => ({ sessionDetection: { enabled: true, agents: ["claude-code"] } }),
    write: async (scope, _dir, config) => {
      store.set(scope, config);
    },
    exists: async (scope) => store.has(scope),
    ...overrides,
  };
}

export function mockHandoffStore(
  initial: HandoffEnvelope[] = [],
): HandoffStorePort {
  const envelopes = new Map<string, HandoffEnvelope>();
  for (const e of initial) {
    envelopes.set(e.handoff.id, e);
  }

  return {
    create: async (handoff: Handoff) => {
      envelopes.set(handoff.id, { handoff, status: "pending" });
      return handoff.id;
    },
    get: async (id: string) => envelopes.get(id),
    getLatestPending: async () => {
      const pending = [...envelopes.values()]
        .filter((e) => e.status === "pending")
        .sort((a, b) => b.handoff.timestamp.localeCompare(a.handoff.timestamp));
      return pending[0];
    },
    listIds: async () => [...envelopes.keys()].sort().reverse(),
    list: async (filter) => {
      let all = [...envelopes.values()];
      if (filter?.status) {
        all = all.filter((e) => e.status === filter.status);
      }
      return all.sort((a, b) =>
        b.handoff.timestamp.localeCompare(a.handoff.timestamp),
      );
    },
    delete: async (id: string) => {
      envelopes.delete(id);
    },
    updateStatus: async (
      id: string,
      status: HandoffStatus,
      meta,
    ) => {
      const existing = envelopes.get(id);
      if (!existing) return undefined;
      const updated: HandoffEnvelope = {
        ...existing,
        status,
        ...(meta?.pickedUpBy && {
          pickedUpBy: meta.pickedUpBy,
          pickedUpAt: new Date().toISOString(),
        }),
        ...(meta?.completedAt && { completedAt: meta.completedAt }),
        ...(meta?.report && { report: meta.report }),
      };
      envelopes.set(id, updated);
      return updated;
    },
  };
}

export function mockCass(
  overrides: Partial<CassPort> = {},
): CassPort {
  return {
    isAvailable: async () => true,
    hasBinary: async () => true,
    indexOnce: async () => {},
    search: async (query): Promise<CassSearchResponse> => ({
      query,
      count: 0,
      totalMatches: 0,
      hits: [],
    }),
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
  session?: HandoffSession,
): SessionDetectPort {
  const defaultSession: HandoffSession = {
    agent: "claude-code",
    sessionId: "test-session-123",
    sourcePath: "/tmp/sessions/test",
  };
  return {
    detect: async () => session ?? defaultSession,
    resolve: async (_cwd, id) => {
      const s = session ?? defaultSession;
      return s.sessionId.startsWith(id) ? s : undefined;
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
        milestones: input.milestones,
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
        skillName: input.skillName,
        verificationSteps: input.verificationSteps,
        dependsOn: input.dependsOn ?? [],
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
