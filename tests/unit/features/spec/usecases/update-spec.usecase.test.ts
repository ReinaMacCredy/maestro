import { describe, it, expect } from "bun:test";
import { createSpec } from "@/features/spec/usecases/create-spec.usecase.js";
import { updateSpec } from "@/features/spec/usecases/update-spec.usecase.js";
import { MaestroError } from "@/shared/errors.js";
import type { SpecStorePort } from "@/features/spec/ports/storage.js";
import type { Spec } from "@/features/spec/domain/types.js";

function mockSpecStore(initial: Spec[] = []): SpecStorePort {
  const store = new Map(initial.map((s) => [s.mission_id, s]));
  return {
    write: async (spec) => { store.set(spec.mission_id, spec); },
    read: async (missionId) => store.get(missionId),
    list: async () => [...store.values()].sort((a, b) => a.mission_id.localeCompare(b.mission_id)),
  };
}

describe("updateSpec", () => {
  it("throws when no spec exists for the mission", async () => {
    const store = mockSpecStore();
    await expect(
      updateSpec(store, "no-such-mission", { acceptance_criteria: [{ text: "A" }] }),
    ).rejects.toBeInstanceOf(MaestroError);
  });

  it("updates acceptance criteria while preserving non_goals", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-001",
      acceptance_criteria: [{ text: "Old criterion" }],
      non_goals: [{ text: "Out of scope" }],
    });

    const updated = await updateSpec(store, "2026-05-04-001", {
      acceptance_criteria: [{ text: "New criterion" }],
    });

    expect(updated.acceptance_criteria).toHaveLength(1);
    expect(updated.acceptance_criteria[0]!.text).toBe("New criterion");
    expect(updated.non_goals[0]!.text).toBe("Out of scope");
  });

  it("preserves criterion ids when they are provided in the update", async () => {
    const store = mockSpecStore();
    const created = await createSpec(store, {
      mission_id: "2026-05-04-002",
      acceptance_criteria: [{ text: "Original" }],
    });
    const originalId = created.acceptance_criteria[0]!.id;

    const updated = await updateSpec(store, "2026-05-04-002", {
      acceptance_criteria: [{ id: originalId, text: "Updated text" }],
    });

    expect(updated.acceptance_criteria[0]!.id).toBe(originalId);
    expect(updated.acceptance_criteria[0]!.text).toBe("Updated text");
  });

  it("assigns fresh ids to criteria that lack them", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-003",
      acceptance_criteria: [{ text: "Old" }],
    });

    const updated = await updateSpec(store, "2026-05-04-003", {
      acceptance_criteria: [{ text: "New without id" }],
    });

    expect(typeof updated.acceptance_criteria[0]!.id).toBe("string");
    expect(updated.acceptance_criteria[0]!.id.length).toBeGreaterThan(0);
  });

  it("round-trips correctly: read back matches the update result", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-004",
      acceptance_criteria: [{ text: "A" }, { text: "B" }],
    });

    const updated = await updateSpec(store, "2026-05-04-004", {
      acceptance_criteria: [{ text: "X" }, { text: "Y" }, { text: "Z" }],
    });

    const readBack = await store.read("2026-05-04-004");
    expect(readBack).toBeDefined();
    expect(readBack!.acceptance_criteria).toHaveLength(3);
    for (let i = 0; i < 3; i++) {
      expect(readBack!.acceptance_criteria[i]!.id).toBe(updated.acceptance_criteria[i]!.id);
      expect(readBack!.acceptance_criteria[i]!.text).toBe(updated.acceptance_criteria[i]!.text);
    }
  });
});
