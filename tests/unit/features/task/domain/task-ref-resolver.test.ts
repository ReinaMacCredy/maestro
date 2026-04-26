import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { createTask } from "@/features/task/usecases/create-task.usecase.js";
import {
  closestSlugSuggestion,
  resolveTaskRef,
} from "@/features/task/domain/task-slug.js";
import { MaestroError } from "@/shared/errors.js";

describe("resolveTaskRef", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-ref-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("R1: tsk-XXX is short-circuited to id lookup", async () => {
    const task = await createTask(store, { title: "X", type: "feature" });
    const found = await resolveTaskRef(store, task.id);
    expect(found.id).toBe(task.id);
  });

  it("falls through to slug lookup for non-id input", async () => {
    const task = await createTask(store, { title: "Add login form", type: "feature" });
    const found = await resolveTaskRef(store, "implement/add-login-form");
    expect(found.id).toBe(task.id);
  });

  it("R2: surfaces a Levenshtein-1 suggestion on miss", async () => {
    await createTask(store, { title: "Add login form", type: "feature" });

    let caught: unknown;
    try {
      await resolveTaskRef(store, "implement/add-login-frm");
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(MaestroError);
    const err = caught as MaestroError;
    expect(err.message).toContain("No task found for slug 'implement/add-login-frm'");
    const text = [err.message, ...(err.hints ?? [])].join("\n");
    expect(text).toContain("implement/add-login-form");
  });

  it("R3: requires strict lowercase match", async () => {
    await createTask(store, { title: "Add login form", type: "feature" });
    await expect(
      resolveTaskRef(store, "Implement/Add-Login-Form"),
    ).rejects.toThrow(MaestroError);
  });

  it("throws taskNotFound on a missing tsk-id", async () => {
    await expect(resolveTaskRef(store, "tsk-000000")).rejects.toThrow(/not found/);
  });
});

describe("closestSlugSuggestion", () => {
  it("returns the closest slug within distance 1", () => {
    const candidates = ["implement/foo", "implement/bar", "fix/baz"];
    expect(closestSlugSuggestion("implement/fo", candidates)).toBe("implement/foo");
    expect(closestSlugSuggestion("implement/foO", candidates)).toBe("implement/foo");
  });

  it("returns undefined when no candidate is within distance 1", () => {
    expect(closestSlugSuggestion("implement/xxxxxx", ["implement/foo"])).toBeUndefined();
  });
});
