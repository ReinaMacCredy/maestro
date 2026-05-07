import { describe, it, expect } from "bun:test";
import { createSpec } from "@/features/spec/usecases/create-spec.usecase.js";
import { getSpec } from "@/features/spec/usecases/get-spec.usecase.js";
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

describe("getSpec", () => {
  it("returns undefined when no spec exists", async () => {
    const store = mockSpecStore();
    const result = await getSpec(store, "2026-05-04-099");
    expect(result).toBeUndefined();
  });

  it("returns the spec for a known mission", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-001",
      acceptance_criteria: [{ text: "Criterion A" }, { text: "Criterion B" }],
    });

    const result = await getSpec(store, "2026-05-04-001");
    expect(result).toBeDefined();
    expect(result!.mission_id).toBe("2026-05-04-001");
    expect(result!.acceptance_criteria).toHaveLength(2);
  });

  it("criterion ids are stable across multiple reads", async () => {
    const store = mockSpecStore();
    await createSpec(store, {
      mission_id: "2026-05-04-002",
      acceptance_criteria: [
        { text: "Criterion 1" },
        { text: "Criterion 2" },
        { text: "Criterion 3" },
      ],
    });

    const first = await getSpec(store, "2026-05-04-002");
    const second = await getSpec(store, "2026-05-04-002");

    expect(first).toBeDefined();
    expect(second).toBeDefined();
    for (let i = 0; i < 3; i++) {
      expect(first!.acceptance_criteria[i]!.id).toBe(second!.acceptance_criteria[i]!.id);
    }
  });
});
