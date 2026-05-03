import { describe, it, expect } from "bun:test";
import { createSpec } from "@/features/spec/usecases/create-spec.usecase.js";
import { CRITERION_ID_PATTERN } from "@/features/spec/domain/spec-id.js";
import type { SpecStorePort } from "@/features/spec/ports/storage.js";
import type { Spec } from "@/features/spec/domain/types.js";

function mockSpecStore(initial: Spec[] = []): SpecStorePort & { written(): Spec[] } {
  const store = new Map(initial.map((s) => [s.mission_id, s]));
  return {
    write: async (spec) => { store.set(spec.mission_id, spec); },
    read: async (missionId) => store.get(missionId),
    list: async () => [...store.values()].sort((a, b) => a.mission_id.localeCompare(b.mission_id)),
    written: () => [...store.values()],
  };
}

describe("createSpec", () => {
  it("creates a spec with generated criterion ids", async () => {
    const store = mockSpecStore();
    const spec = await createSpec(store, {
      mission_id: "2026-05-04-001",
      acceptance_criteria: [
        { text: "Tests pass" },
        { text: "Build succeeds" },
        { text: "No lint errors" },
      ],
    });

    expect(spec.mission_id).toBe("2026-05-04-001");
    expect(spec.schema_version).toBe(1);
    expect(spec.acceptance_criteria).toHaveLength(3);
    expect(spec.acceptance_criteria[0]!.text).toBe("Tests pass");
    expect(spec.acceptance_criteria[1]!.text).toBe("Build succeeds");
    expect(spec.acceptance_criteria[2]!.text).toBe("No lint errors");
  });

  it("assigns stable CRITERION_ID_PATTERN ids to each criterion", async () => {
    const store = mockSpecStore();
    const spec = await createSpec(store, {
      mission_id: "2026-05-04-002",
      acceptance_criteria: [
        { text: "Criterion A" },
        { text: "Criterion B" },
      ],
    });

    for (const c of spec.acceptance_criteria) {
      expect(CRITERION_ID_PATTERN.test(c.id)).toBe(true);
    }
  });

  it("persists to the store", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-003",
      acceptance_criteria: [{ text: "A" }],
    });

    const read = await store.read("2026-05-04-003");
    expect(read).toBeDefined();
    expect(read!.acceptance_criteria[0]!.text).toBe("A");
  });

  it("criterion ids are stable across reads", async () => {
    const store = mockSpecStore();
    const created = await createSpec(store, {
      mission_id: "2026-05-04-004",
      acceptance_criteria: [
        { text: "First" },
        { text: "Second" },
        { text: "Third" },
      ],
    });

    const read = await store.read("2026-05-04-004");
    expect(read).toBeDefined();
    for (let i = 0; i < 3; i++) {
      expect(read!.acceptance_criteria[i]!.id).toBe(created.acceptance_criteria[i]!.id);
    }
  });

  it("defaults non_goals to empty array when not provided", async () => {
    const store = mockSpecStore();
    const spec = await createSpec(store, {
      mission_id: "2026-05-04-005",
      acceptance_criteria: [{ text: "Done" }],
    });
    expect(spec.non_goals).toEqual([]);
    expect(spec.runtime_signals).toEqual([]);
  });
});
