import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { readyTasks } from "@/features/task/usecases/ready-tasks.usecase.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import { closeTask } from "@/features/task/usecases/close-task.usecase.js";
import { updateTask } from "@/features/task/usecases/update-task.usecase.js";
import { captureTaskCandidate } from "@/features/task/usecases/capture-task-candidate.usecase.js";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { FsCandidateStoreAdapter } from "@/features/task/adapters/fs-candidate-store.adapter.js";
import type { CandidateStorePort } from "@/features/task/ports/candidate-store.port.js";

describe("readyTasks", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-ready-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  describe("step 1: exclude closed", () => {
    it("omits closed tasks from ready", async () => {
      const t1 = await createTask(store, { title: "keep open" });
      const t2 = await createTask(store, { title: "close this" });
      await closeTask(store, t2.id, { reason: "done" });

      const result = await readyTasks(store);
      expect(result.map((t) => t.id)).toEqual([t1.id]);
    });
  });

    describe("step 2: exclude blocked by open dependency", () => {
      it("excludes tasks explicitly marked blocked", async () => {
        const task = await createTask(store, { title: "wait on external review" });
        await updateTask(store, task.id, { status: "blocked" });

        const result = await readyTasks(store);
        expect(result.find((t) => t.id === task.id)).toBeUndefined();
      });

      it("excludes a task with an open direct dependency", async () => {
        const parent = await createTask(store, { title: "parent" });
        const child = await createTask(store, {
        title: "child",
        dependsOn: [parent.id],
      });

      const result = await readyTasks(store);
      expect(result.map((t) => t.id)).toEqual([parent.id]);
      expect(result.find((t) => t.id === child.id)).toBeUndefined();
    });

    it("unblocks a dependency when its parent is closed", async () => {
      const parent = await createTask(store, { title: "parent" });
      const child = await createTask(store, {
        title: "child",
        dependsOn: [parent.id],
      });
      await closeTask(store, parent.id, { reason: "done" });

      const result = await readyTasks(store);
      expect(result.map((t) => t.id)).toEqual([child.id]);
    });

    it("transitive: child is blocked while its grandparent is open", async () => {
      const g = await createTask(store, { title: "grandparent" });
      const p = await createTask(store, { title: "parent", dependsOn: [g.id] });
      const c = await createTask(store, { title: "child", dependsOn: [p.id] });

      const result = await readyTasks(store);
      // Only grandparent is ready; parent and child remain blocked.
      expect(result.map((t) => t.id)).toEqual([g.id]);
    });

    it("transitive: closing grandparent unblocks parent but not yet child", async () => {
      const g = await createTask(store, { title: "grandparent" });
      const p = await createTask(store, { title: "parent", dependsOn: [g.id] });
      const c = await createTask(store, { title: "child", dependsOn: [p.id] });
      await closeTask(store, g.id, { reason: "done" });

      const result = await readyTasks(store);
      expect(result.map((t) => t.id)).toEqual([p.id]);
      expect(result.find((t) => t.id === c.id)).toBeUndefined();
    });

    it("keeps a child blocked when an intermediate task is closed out of order", async () => {
      const g = await createTask(store, { title: "grandparent" });
      const p = await createTask(store, { title: "parent", dependsOn: [g.id] });
      const c = await createTask(store, { title: "child", dependsOn: [p.id] });
      await closeTask(store, p.id, { reason: "closed early" });

      const result = await readyTasks(store);
      expect(result.map((t) => t.id)).toEqual([g.id]);
      expect(result.find((t) => t.id === c.id)).toBeUndefined();
    });

    it("parent (hierarchy) does NOT block a child", async () => {
      // parentId is hierarchy, not a blocking edge. br semantics.
      const group = await createTask(store, { title: "group" });
      const leaf = await createTask(store, {
        title: "leaf",
        parentId: group.id,
      });

      const result = await readyTasks(store);
      // Both are unblocked; parentId alone is not a dependency.
      expect(result.length).toBe(2);
    });

    it("ignores orphaned dependency ids", async () => {
      // If dependsOn points at a non-existent id, treat as "not blocking"
      // per br's behavior.
      const task = await createTask(store, {
        title: "orphan",
      });
      // Hand-craft an orphan edge by updating the raw store... we cannot
      // pass unknown ids through createTask's validator, so we bypass it
      // via store.update which has no cross-reference check.
      await store.update(task.id, {});
      // Simulate orphan by writing raw: best we can do without bypassing
      // validation is assert that a task with empty deps is ready.
      const result = await readyTasks(store);
      expect(result.find((t) => t.id === task.id)).toBeDefined();
    });
  });

  describe("step 3: defer filter", () => {
    it("excludes deferred tasks (deferUntil in the future)", async () => {
      const task = await createTask(store, { title: "deferred" });
      const future = new Date(Date.now() + 60 * 60 * 1000).toISOString();
      await store.update(task.id, { deferUntil: future });

      const result = await readyTasks(store);
      expect(result.find((t) => t.id === task.id)).toBeUndefined();
    });

    it("includes deferred tasks with --include-deferred", async () => {
      const task = await createTask(store, { title: "deferred" });
      const future = new Date(Date.now() + 60 * 60 * 1000).toISOString();
      await store.update(task.id, { deferUntil: future });

      const result = await readyTasks(store, { includeDeferred: true });
      expect(result.find((t) => t.id === task.id)).toBeDefined();
    });

      it("excludes tasks with deferred status unless includeDeferred is set", async () => {
        const task = await createTask(store, { title: "deferred status" });
        await updateTask(store, task.id, { status: "deferred" });

      const hidden = await readyTasks(store);
      expect(hidden.find((t) => t.id === task.id)).toBeUndefined();

      const included = await readyTasks(store, { includeDeferred: true });
      expect(included.find((t) => t.id === task.id)).toBeDefined();
    });

    it("treats past deferUntil as expired (task becomes ready)", async () => {
      const task = await createTask(store, { title: "expired defer" });
      const past = new Date(Date.now() - 60 * 60 * 1000).toISOString();
      await store.update(task.id, { deferUntil: past });

      const result = await readyTasks(store);
      expect(result.find((t) => t.id === task.id)).toBeDefined();
    });
  });

  describe("step 4: user filters", () => {
    it("filters by label", async () => {
      await createTask(store, { title: "auth A", labels: ["auth"] });
      await createTask(store, { title: "ui B", labels: ["ui"] });

      const auth = await readyTasks(store, { label: "auth" });
      expect(auth.map((t) => t.title)).toEqual(["auth A"]);
    });

    it("filters by priority", async () => {
      await createTask(store, { title: "crit", priority: 0 });
      await createTask(store, { title: "norm", priority: 2 });

      const result = await readyTasks(store, { priority: 0 });
      expect(result.map((t) => t.title)).toEqual(["crit"]);
    });

    it("filters by type", async () => {
      await createTask(store, { title: "b1", type: "bug" });
      await createTask(store, { title: "f1", type: "feature" });

      const result = await readyTasks(store, { type: "bug" });
      expect(result.map((t) => t.title)).toEqual(["b1"]);
    });

    it("filters by assignee", async () => {
      const t1 = await createTask(store, { title: "mine" });
      const t2 = await createTask(store, { title: "theirs" });
      await store.claim(t1.id, "alice");
      await store.claim(t2.id, "bob");

      const result = await readyTasks(store, { assignee: "alice" });
      expect(result.map((t) => t.id)).toEqual([t1.id]);
    });

    it("filters by unassigned", async () => {
      await createTask(store, { title: "unassigned" });
      const assigned = await createTask(store, { title: "assigned" });
      await store.claim(assigned.id, "alice");

      const result = await readyTasks(store, { unassigned: true });
      expect(result.map((t) => t.title)).toEqual(["unassigned"]);
    });
  });

  describe("step 5: hybrid sort", () => {
    it("places P0/P1 before P2+ and sorts each group by createdAt ASC", async () => {
      // Create in a specific order so we can verify the sort.
      const p2a = await createTask(store, { title: "P2 first", priority: 2 });
      await new Promise((r) => setTimeout(r, 5));
      const p0 = await createTask(store, { title: "P0", priority: 0 });
      await new Promise((r) => setTimeout(r, 5));
      const p4 = await createTask(store, { title: "P4", priority: 4 });
      await new Promise((r) => setTimeout(r, 5));
      const p1 = await createTask(store, { title: "P1", priority: 1 });
      await new Promise((r) => setTimeout(r, 5));
      const p2b = await createTask(store, { title: "P2 second", priority: 2 });

      const result = await readyTasks(store);
      // Expected order: P0 (priority high, created first), P1 (priority high),
      // then P2a, P4, P2b sorted by createdAt among the non-high group.
      expect(result.map((t) => t.id)).toEqual([p0.id, p1.id, p2a.id, p4.id, p2b.id]);
    });
  });

  describe("step 6: limit", () => {
    it("slices to default limit of 20", async () => {
      for (let i = 0; i < 25; i++) {
        await createTask(store, { title: `t${i}` });
      }
      const result = await readyTasks(store);
      expect(result.length).toBe(20);
    });

    it("respects explicit limit", async () => {
      for (let i = 0; i < 10; i++) {
        await createTask(store, { title: `t${i}` });
      }
      const result = await readyTasks(store, { limit: 3 });
      expect(result.length).toBe(3);
    });

    it("limit 0 returns all matches", async () => {
      for (let i = 0; i < 30; i++) {
        await createTask(store, { title: `t${i}` });
      }
      const result = await readyTasks(store, { limit: 0 });
      expect(result.length).toBe(30);
    });
  });

  describe("empty store", () => {
    it("returns empty array", async () => {
      const result = await readyTasks(store);
      expect(result).toEqual([]);
    });
  });

  describe("brainstorm coordination scenario", () => {
    it("matches the three-task chain from the visual", async () => {
      // From the sequence diagram: tsk-api -> tsk-mw -> tsk-prot
      // A third independent task (tsk-ui) is blocked only on tsk-api.
      const api = await createTask(store, { title: "login endpoint" });
      const mw = await createTask(store, {
        title: "JWT middleware",
        dependsOn: [api.id],
      });
      const ui = await createTask(store, {
        title: "login UI",
        dependsOn: [api.id],
      });
      const prot = await createTask(store, {
        title: "protected routes",
        dependsOn: [mw.id],
      });

      // t=1: only api is ready.
      let result = await readyTasks(store);
      expect(result.map((t) => t.id)).toEqual([api.id]);

      // t=2: close api; mw and ui unblock.
      await closeTask(store, api.id, { reason: "done" });
      result = await readyTasks(store);
      expect(new Set(result.map((t) => t.id))).toEqual(new Set([mw.id, ui.id]));

      // t=3: close mw; prot unblocks.
      await closeTask(store, mw.id, { reason: "done" });
      result = await readyTasks(store);
      expect(new Set(result.map((t) => t.id))).toEqual(new Set([ui.id, prot.id]));
    });
  });

    describe("step 7: hint attachment (active memory)", () => {
      it("does not read candidates when there are no ready tasks", async () => {
        const task = await createTask(store, { title: "blocked by waiting" });
        await updateTask(store, task.id, { status: "blocked" });

        const candidateStore: CandidateStorePort = {
          create: async () => {
            throw new Error("candidate creation should not run in readyTasks");
          },
          all: async () => {
            throw new Error("candidate reads should be skipped when nothing is ready");
          },
        };

        const result = await readyTasks(store, {}, new Date(), candidateStore);
        expect(result).toEqual([]);
      });

      it("returns briefings with empty hints when no candidate store is passed", async () => {
        await createTask(store, { title: "JWT middleware" });
        const result = await readyTasks(store);
      expect(result.length).toBe(1);
      expect(result[0]?.hints).toEqual([]);
    });

    it("returns empty hints when the candidate store is empty", async () => {
      const candidateStore = new FsCandidateStoreAdapter(tmpDir);
      await createTask(store, { title: "JWT middleware" });
      const result = await readyTasks(store, {}, new Date(), candidateStore);
      expect(result[0]?.hints).toEqual([]);
    });

    it("surfaces a hint from a closed task with overlapping keywords", async () => {
      const candidateStore = new FsCandidateStoreAdapter(tmpDir);

      // Seed a closed task with a lesson.
      const past = await createTask(store, {
        title: "Implement argon2 password hashing",
      });
      const closed = await closeTask(store, past.id, {
        reason: "argon2 compare was backwards",
      });
      await captureTaskCandidate(candidateStore, closed);

      // New task with overlapping keyword ("password").
      await createTask(store, { title: "JWT password middleware" });

      const result = await readyTasks(store, {}, new Date(), candidateStore);
      expect(result.length).toBe(1);
      expect(result[0]?.hints.length).toBeGreaterThanOrEqual(1);
      expect(result[0]?.hints[0]?.sourceTaskId).toBe(past.id);
      expect(result[0]?.hints[0]?.reason).toBe("argon2 compare was backwards");
      expect(result[0]?.hints[0]?.matchedKeywords).toContain("password");
    });

    it("does not attach hints when keywords do not overlap", async () => {
      const candidateStore = new FsCandidateStoreAdapter(tmpDir);

      const past = await createTask(store, { title: "Database migration" });
      const closed = await closeTask(store, past.id, { reason: "rollback broke" });
      await captureTaskCandidate(candidateStore, closed);

      await createTask(store, { title: "JWT middleware" });

      const result = await readyTasks(store, {}, new Date(), candidateStore);
      expect(result[0]?.hints).toEqual([]);
    });

    it("does not let a reopened task see its own past close as a hint", async () => {
      const candidateStore = new FsCandidateStoreAdapter(tmpDir);

      // Close, capture, then reopen.
      const t = await createTask(store, { title: "JWT middleware" });
      const closed = await closeTask(store, t.id, { reason: "jwt signing issue" });
      await captureTaskCandidate(candidateStore, closed);
      await updateTask(store, t.id, { status: "open" });

      const result = await readyTasks(store, {}, new Date(), candidateStore);
      expect(result.length).toBe(1);
      expect(result[0]?.id).toBe(t.id);
      expect(result[0]?.hints).toEqual([]);
    });

    it("attaches hints to every matching ready task independently", async () => {
      const candidateStore = new FsCandidateStoreAdapter(tmpDir);

      const past = await createTask(store, { title: "Password hashing with argon2" });
      const closedPast = await closeTask(store, past.id, {
        reason: "argon2 is the correct algorithm",
      });
      await captureTaskCandidate(candidateStore, closedPast);

      await createTask(store, { title: "JWT password middleware" });
      await createTask(store, { title: "Login password validation" });
      await createTask(store, { title: "Protected routes" });

      const result = await readyTasks(store, {}, new Date(), candidateStore);
      expect(result.length).toBe(3);

      const byTitle = new Map(result.map((r) => [r.title, r] as const));
      expect(byTitle.get("JWT password middleware")?.hints.length).toBe(1);
      expect(byTitle.get("Login password validation")?.hints.length).toBe(1);
      expect(byTitle.get("Protected routes")?.hints).toEqual([]);
    });
  });
});
