import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlMissionStore } from "@/repo/jsonl-mission-store.adapter.js";
import {
  DuplicateMissionSlugError,
  MissionNotFoundError,
} from "@/repo/mission-store.port.js";

const FROZEN = new Date("2026-05-15T10:00:00.000Z");

function makeStore(root: string): JsonlMissionStore {
  let n = 0;
  return new JsonlMissionStore({
    repoRoot: root,
    clock: () => FROZEN,
    idFactory: () => `pln-${++n}`,
  });
}

describe("JsonlMissionStore", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-plan-store-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("returns empty list when no plans file exists", async () => {
    const store = makeStore(root);
    expect(await store.list()).toEqual([]);
  });

  it("creates, reads back, and updates a plan", async () => {
    const store = makeStore(root);
    const created = await store.create({
      slug: "alpha",
      title: "Alpha",
      state: "specified",
      spec_path: ".maestro/specs/alpha.md",
    });
    expect(created.id).toBe("pln-1");
    expect(created.state).toBe("specified");
    expect(created.spec_path).toBe(".maestro/specs/alpha.md");
    expect(created.created_at).toBe(FROZEN.toISOString());

    const fetched = await store.get("pln-1");
    expect(fetched).toEqual(created);

    const updated = await store.update("pln-1", { state: "planned" });
    expect(updated.state).toBe("planned");
    expect(updated.id).toBe("pln-1");
    expect(updated.slug).toBe("alpha");
  });

  it("rejects duplicate slugs", async () => {
    const store = makeStore(root);
    await store.create({ slug: "alpha", title: "Alpha", state: "specified" });
    await expect(
      store.create({ slug: "alpha", title: "Alpha again", state: "specified" }),
    ).rejects.toBeInstanceOf(DuplicateMissionSlugError);
  });

  it("throws MissionNotFoundError on update of missing id", async () => {
    const store = makeStore(root);
    await expect(store.update("pln-missing", { state: "planned" })).rejects.toBeInstanceOf(
      MissionNotFoundError,
    );
  });

  it("persists each plan as one JSONL line", async () => {
    const store = makeStore(root);
    await store.create({ slug: "alpha", title: "Alpha", state: "specified" });
    await store.create({ slug: "beta", title: "Beta", state: "specified" });
    const text = await readFile(join(root, ".maestro/missions/plans.jsonl"), "utf8");
    const lines = text.trim().split("\n");
    expect(lines.length).toBe(2);
    const first = JSON.parse(lines[0]) as { id: string; slug: string };
    expect(first.slug).toBe("alpha");
  });

  it("listByState filters by state", async () => {
    const store = makeStore(root);
    await store.create({ slug: "alpha", title: "Alpha", state: "specified" });
    const planned = await store.create({ slug: "beta", title: "Beta", state: "specified" });
    await store.update(planned.id, { state: "planned" });

    const planned_results = await store.listByState("planned");
    expect(planned_results.length).toBe(1);
    expect(planned_results[0].slug).toBe("beta");

    const specified = await store.listByState("specified");
    expect(specified.length).toBe(1);
    expect(specified[0].slug).toBe("alpha");
  });

  it("serializes writes (FIFO ordering via internal queue)", async () => {
    const store = makeStore(root);
    const results = await Promise.all([
      store.create({ slug: "a", title: "A", state: "specified" }),
      store.create({ slug: "b", title: "B", state: "specified" }),
      store.create({ slug: "c", title: "C", state: "specified" }),
    ]);
    expect(results.map((r) => r.id)).toEqual(["pln-1", "pln-2", "pln-3"]);
    const plans = await store.list();
    expect(plans.map((p) => p.slug)).toEqual(["a", "b", "c"]);
  });
});
