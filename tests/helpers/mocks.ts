import type { GitPort } from "../../src/ports/git.port.js";
import type { ConfigPort } from "../../src/ports/config.port.js";
import type { HandoffStorePort } from "../../src/ports/handoff-store.port.js";
import type { CassPort } from "../../src/ports/cass.port.js";
import type { SessionDetectPort } from "../../src/ports/session-detect.port.js";
import type { NotesStorePort } from "../../src/ports/notes-store.port.js";
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
    updateStatus: async (
      id: string,
      status: HandoffStatus,
      meta,
    ) => {
      const existing = envelopes.get(id);
      if (!existing) throw new Error(`Handoff ${id} not found`);
      envelopes.set(id, {
        ...existing,
        status,
        ...(meta?.pickedUpBy && {
          pickedUpBy: meta.pickedUpBy,
          pickedUpAt: new Date().toISOString(),
        }),
        ...(meta?.completedAt && { completedAt: meta.completedAt }),
        ...(meta?.report && { report: meta.report }),
      });
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
  return {
    detect: async () =>
      session ?? {
        agent: "claude-code",
        sessionId: "test-session-123",
        sourcePath: "/tmp/sessions/test",
        cassIndexed: false,
      },
  };
}
