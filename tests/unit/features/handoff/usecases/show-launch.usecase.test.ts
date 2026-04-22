import { describe, expect, it } from "bun:test";
import { showLaunch } from "@/features/handoff";
import { MaestroError } from "@/shared/errors.js";
import { makeHandoffLaunchRecord, mockLaunchStore } from "../../../../helpers/mocks.js";

describe("showLaunch", () => {
  it("returns the matching packet", async () => {
    const record = makeHandoffLaunchRecord({ id: "crimson-fox-1", createdAt: "2026-04-22T00:00:00.000Z" });
    const result = await showLaunch(mockLaunchStore([record]), "crimson-fox-1");
    expect(result.id).toBe("crimson-fox-1");
  });

  it("throws MaestroError when the packet does not exist", async () => {
    const store = mockLaunchStore([]);
    await expect(showLaunch(store, "missing-id-9")).rejects.toThrow(MaestroError);
  });
});
