import { describe, expect, it } from "bun:test";
import { buildMemoryStats, getMemoryStats } from "@/features/memory";
import { compileLearnings } from "@/features/memory/usecases/memory-compile.usecase.js";
import { appendLearning } from "@/features/memory/usecases/memory-learn.usecase.js";
import { searchMemory } from "@/features/memory/usecases/memory-search.usecase.js";
import {
  mockCorrectionStore,
  mockGit,
  mockLearningStore,
  mockProjectGraphStore,
  mockRatchetStore,
} from "../../../../helpers/mocks.js";

describe("compileLearnings", () => {
  it("writes compiled learnings and returns the raw entries", async () => {
    const store = mockLearningStore([
      { sessionDate: "2026-04-15", content: "Use smaller seams", branch: "main" },
      { sessionDate: "2026-04-16", content: "Cover edge cases", branch: "feat/tests" },
    ]);

    const result = await compileLearnings(store, "Coverage summary");

    expect(result.rawEntries).toHaveLength(2);
    expect(result.compiled.summary).toBe("Coverage summary");
    expect(result.compiled.rawCount).toBe(2);
    expect(typeof result.compiled.compiledAt).toBe("string");
    expect(await store.readCompiled()).toEqual(result.compiled);
  });
});

describe("appendLearning", () => {
  it("stores the current git branch when the directory is a repo", async () => {
    const store = mockLearningStore();
    const git = mockGit({
      isRepo: async (dir) => {
        expect(dir).toBe("/repo");
        return true;
      },
      getState: async () => ({
        branch: "feat/coverage",
        recentCommits: [],
        changedFiles: [],
        workingTreeClean: true,
        diffStat: "+0 -0",
      }),
    });

    const entry = await appendLearning(git, store, {
      content: "Add coverage for session command",
      dir: "/repo",
    });

    expect(entry.content).toBe("Add coverage for session command");
    expect(entry.branch).toBe("feat/coverage");
    expect(entry.sessionDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(await store.listRaw()).toEqual([entry]);
  });

  it("omits the branch when the directory is not a repo", async () => {
    const store = mockLearningStore();
    const git = mockGit({
      isRepo: async () => false,
      getState: async () => {
        throw new Error("should not be called");
      },
    });

    const entry = await appendLearning(git, store, {
      content: "Works outside git too",
      dir: "/tmp/no-repo",
    });

    expect(entry.branch).toBeUndefined();
  });
});

describe("searchMemory", () => {
  it("returns matching corrections and case-insensitive learning content matches", async () => {
    const corrections = mockCorrectionStore([
      {
          id: "corr-1",
          rule: "Never skip coverage checks",
          source: "user",
          severity: "hard",
          trigger: { keywords: ["coverage", "tests"], fileGlobs: [] },
          createdAt: "2026-04-15T00:00:00.000Z",
          updatedAt: "2026-04-15T00:00:00.000Z",
        },
      {
          id: "corr-2",
          rule: "Different topic",
          source: "user",
          severity: "soft",
          trigger: { keywords: ["other"], fileGlobs: [] },
          createdAt: "2026-04-16T00:00:00.000Z",
          updatedAt: "2026-04-16T00:00:00.000Z",
        },
    ]);
    const learnings = mockLearningStore([
      { sessionDate: "2026-04-15", content: "Coverage work is incomplete", branch: "main" },
      { sessionDate: "2026-04-16", content: "No match here", branch: "main" },
    ]);

    const result = await searchMemory(corrections, learnings, "coverage");

    expect(result.corrections).toHaveLength(1);
    expect(result.corrections[0]?.id).toBe("corr-1");
    expect(result.learnings).toEqual([
      { sessionDate: "2026-04-15", content: "Coverage work is incomplete", branch: "main" },
    ]);
  });
});

describe("buildMemoryStats", () => {
  it("derives hard and soft correction counts plus a passing ratchet baseline", () => {
    const stats = buildMemoryStats({
      corrections: [
        {
            id: "corr-1",
            rule: "Hard rule",
            source: "user",
            severity: "hard",
            trigger: { keywords: ["coverage"], fileGlobs: [] },
            createdAt: "2026-04-15T00:00:00.000Z",
            updatedAt: "2026-04-15T00:00:00.000Z",
          },
        {
            id: "corr-2",
            rule: "Soft rule",
            source: "user",
            severity: "soft",
            trigger: { keywords: ["notes"], fileGlobs: [] },
            createdAt: "2026-04-16T00:00:00.000Z",
            updatedAt: "2026-04-16T00:00:00.000Z",
          },
      ],
      rawLearningCount: 4,
      compiledLearnings: {
        compiledAt: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
        summary: "Compiled",
        rawCount: 4,
      },
        ratchetSuite: {
          assertions: [
            { id: "a1", correctionId: "corr-1", rule: "first", check: "foo", createdAt: "2026-04-16T00:00:00.000Z" },
            { id: "a2", correctionId: "corr-2", rule: "second", check: "bar", createdAt: "2026-04-16T00:00:00.000Z" },
          ],
        },
        ratchetBaseline: {
          lastRunAt: "2026-04-16T00:00:00.000Z",
          passCount: 2,
        },
      graphProjects: 3,
      graphLinks: 5,
    });

    expect(stats.corrections).toEqual({ total: 2, hard: 1, soft: 1 });
    expect(stats.learnings.rawCount).toBe(4);
    expect(stats.learnings.compiledAt).toBeDefined();
    expect(stats.learnings.staleDays).toBeGreaterThanOrEqual(1);
    expect(stats.ratchet).toEqual({ assertions: 2, lastResult: "pass" });
    expect(stats.graph).toEqual({ projects: 3, links: 5 });
  });

  it("returns undefined stale data and ratchet result when compiled/baseline data is absent", () => {
    const stats = buildMemoryStats({
      corrections: [],
      rawLearningCount: 0,
      ratchetSuite: { assertions: [] },
      graphProjects: 0,
      graphLinks: 0,
    });

    expect(stats.learnings.compiledAt).toBeUndefined();
    expect(stats.learnings.staleDays).toBeUndefined();
    expect(stats.ratchet.lastResult).toBeUndefined();
  });
});

describe("getMemoryStats", () => {
  it("loads stores in aggregate and includes graph counts when a graph store is provided", async () => {
    const corrStore = mockCorrectionStore([
      {
          id: "corr-1",
          rule: "Hard rule",
          source: "user",
          severity: "hard",
          trigger: { keywords: ["coverage"], fileGlobs: [] },
          createdAt: "2026-04-15T00:00:00.000Z",
          updatedAt: "2026-04-15T00:00:00.000Z",
        },
    ]);
    const learnStore = mockLearningStore([
      { sessionDate: "2026-04-15", content: "one", branch: "main" },
      { sessionDate: "2026-04-16", content: "two", branch: "main" },
    ]);
    await learnStore.writeCompiled({
      compiledAt: "2026-04-16T00:00:00.000Z",
      summary: "summary",
      rawCount: 2,
    });
      const ratchetStore = mockRatchetStore(
        {
          assertions: [{ id: "a1", correctionId: "corr-1", rule: "desc", check: "foo", createdAt: "2026-04-16T00:00:00.000Z" }],
        },
        {
          lastRunAt: "2026-04-16T00:00:00.000Z",
          passCount: 0,
        },
      );
    const graphStore = mockProjectGraphStore({
      nodes: [
        { name: "maestro", path: "/code/maestro" },
        { name: "web", path: "/code/web" },
      ],
      edges: [
        { from: "maestro", to: "web", relation: "exposes" },
      ],
    });

    const stats = await getMemoryStats(corrStore, learnStore, ratchetStore, graphStore);

    expect(stats.corrections.total).toBe(1);
    expect(stats.learnings.rawCount).toBe(2);
    expect(stats.ratchet).toEqual({ assertions: 1, lastResult: "fail" });
    expect(stats.graph).toEqual({ projects: 2, links: 1 });
  });

  it("defaults graph counts to zero when no graph store is provided", async () => {
    const stats = await getMemoryStats(
      mockCorrectionStore(),
      mockLearningStore(),
      mockRatchetStore(),
    );

    expect(stats.graph).toEqual({ projects: 0, links: 0 });
  });
});
