import { describe, expect, it } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import {
  mapV1StatusToV2State,
  mapV1TaskToV2,
  normalizeV1Status,
  type V1TaskShape,
} from "@/repo/v1-state-mapping.js";

const FIXTURE_PATH = join(import.meta.dir, "fixtures/v1-tasks.json");

async function loadFixture(): Promise<readonly V1TaskShape[]> {
  const raw = await readFile(FIXTURE_PATH, "utf8");
  return JSON.parse(raw) as readonly V1TaskShape[];
}

describe("normalizeV1Status", () => {
  it("passes through canonical v1 statuses unchanged", () => {
    expect(normalizeV1Status("pending")).toBe("pending");
    expect(normalizeV1Status("in_progress")).toBe("in_progress");
    expect(normalizeV1Status("completed")).toBe("completed");
  });

  it("maps legacy strings to their v1 successors", () => {
    expect(normalizeV1Status("open")).toBe("pending");
    expect(normalizeV1Status("closed")).toBe("completed");
    expect(normalizeV1Status("blocked")).toBe("blocked");
    expect(normalizeV1Status("deferred")).toBe("blocked");
  });

  it("returns undefined for unknown strings", () => {
    expect(normalizeV1Status("weird-future-status")).toBeUndefined();
    expect(normalizeV1Status("")).toBeUndefined();
  });
});

describe("mapV1StatusToV2State", () => {
  it("pending without assignee -> draft", () => {
    expect(mapV1StatusToV2State({ status: "pending" })).toBe("draft");
  });

  it("pending with assignee -> claimed", () => {
    expect(mapV1StatusToV2State({ status: "pending", assignee: "agent-a" })).toBe("claimed");
  });

  it("in_progress -> doing", () => {
    expect(mapV1StatusToV2State({ status: "in_progress" })).toBe("doing");
  });

  it("completed without abandon hint -> shipped", () => {
    expect(mapV1StatusToV2State({ status: "completed", closeReason: "merged in PR 41" })).toBe(
      "shipped",
    );
  });

  it("completed with abandon hint -> abandoned", () => {
    expect(
      mapV1StatusToV2State({ status: "completed", closeReason: "abandoned: no longer relevant" }),
    ).toBe("abandoned");
    expect(
      mapV1StatusToV2State({ status: "completed", closeReason: "Cancelled by product" }),
    ).toBe("abandoned");
    expect(mapV1StatusToV2State({ status: "completed", closeReason: "won't fix" })).toBe(
      "abandoned",
    );
  });

  it("legacy blocked / deferred land at blocked", () => {
    expect(mapV1StatusToV2State({ status: "blocked" })).toBe("blocked");
    expect(mapV1StatusToV2State({ status: "deferred" })).toBe("blocked");
  });

  it("legacy open -> draft when there is no assignee", () => {
    expect(mapV1StatusToV2State({ status: "open" })).toBe("draft");
  });

  it("legacy closed -> shipped", () => {
    expect(mapV1StatusToV2State({ status: "closed" })).toBe("shipped");
  });

  it("unknown status conservatively -> draft", () => {
    expect(mapV1StatusToV2State({ status: "weird-future-status" })).toBe("draft");
  });
});

describe("mapV1TaskToV2 (fixture)", () => {
  it("maps every fixture v1 task to the expected v2 state", async () => {
    const fixture = await loadFixture();
    const byId = new Map(fixture.map((t) => [t.id, t]));

    const mapped = Object.fromEntries(
      fixture.map((t) => [t.id, mapV1TaskToV2(t).state]),
    );

    expect(mapped).toEqual({
      "v1-task-0001": "draft",
      "v1-task-0002": "claimed",
      "v1-task-0003": "doing",
      "v1-task-0004": "shipped",
      "v1-task-0005": "abandoned",
      "v1-task-0006": "blocked",
      "v1-task-0007": "blocked",
      "v1-task-0008": "draft",
      "v1-task-0009": "shipped",
      "v1-task-0010": "draft",
    });

    const claimed = mapV1TaskToV2(byId.get("v1-task-0002")!);
    expect(claimed.assignee).toBe("agent-a");
    expect(claimed.claimed_at).toBe("2026-01-02T09:00:00.000Z");

    const doing = mapV1TaskToV2(byId.get("v1-task-0003")!);
    expect(doing.assignee).toBe("agent-b");
    expect(doing.blocked_by).toEqual(["v1-task-0002"]);

    const abandoned = mapV1TaskToV2(byId.get("v1-task-0005")!);
    expect(abandoned.abandon_reason).toContain("no longer reproducible");

    const blocked = mapV1TaskToV2(byId.get("v1-task-0006")!);
    expect(blocked.block_reason).toBe("needs product input");
  });

  it("preserves v1 id by default, slug, created_at and updated_at", async () => {
    const fixture = await loadFixture();
    const sample = fixture[0];
    const mapped = mapV1TaskToV2(sample);
    expect(mapped.id).toBe(sample.id);
    expect(mapped.slug).toBe(sample.slug!);
    expect(mapped.created_at).toBe(sample.createdAt);
    expect(mapped.updated_at).toBe(sample.updatedAt);
  });

  it("derives a slug from the title when v1 had no slug", () => {
    const result = mapV1TaskToV2({
      id: "v1-no-slug",
      title: "Hello, World! Build the Thing.",
      status: "pending",
      createdAt: "2026-01-01T00:00:00.000Z",
      updatedAt: "2026-01-01T00:00:00.000Z",
    });
    expect(result.slug).toBe("hello-world-build-the-thing");
  });

  it("falls back to id-derived slug when title is empty", () => {
    const result = mapV1TaskToV2({
      id: "abc-XYZ_123",
      title: "",
      status: "pending",
      createdAt: "2026-01-01T00:00:00.000Z",
      updatedAt: "2026-01-01T00:00:00.000Z",
    });
    expect(result.slug).toBe("abc-xyz-123");
  });

  it("idOverride wins over the v1 id when provided", () => {
    const result = mapV1TaskToV2(
      {
        id: "v1-original",
        title: "Anything",
        status: "pending",
        createdAt: "2026-01-01T00:00:00.000Z",
        updatedAt: "2026-01-01T00:00:00.000Z",
      },
      { idOverride: "tsk-fresh" },
    );
    expect(result.id).toBe("tsk-fresh");
  });

  it("records spec_path when specPath option is provided", () => {
    const result = mapV1TaskToV2(
      {
        id: "v1-spec",
        title: "demo",
        status: "pending",
        createdAt: "2026-01-01T00:00:00.000Z",
        updatedAt: "2026-01-01T00:00:00.000Z",
      },
      { specPath: ".maestro/specs/demo.md" },
    );
    expect(result.spec_path).toBe(".maestro/specs/demo.md");
  });
});
