import { describe, expect, it } from "bun:test";
import { listLaunches } from "@/features/handoff";
import { makeHandoffLaunchRecord, mockLaunchStore } from "../../../../helpers/mocks.js";

describe("listLaunches", () => {
  const open1 = makeHandoffLaunchRecord({ id: "alpha-fox-1", createdAt: "2026-04-20T00:00:00.000Z" });
  const open2 = makeHandoffLaunchRecord({ id: "beta-bear-2", createdAt: "2026-04-21T00:00:00.000Z" });
  const consumed = makeHandoffLaunchRecord({
    id: "gamma-owl-3",
    createdAt: "2026-04-22T00:00:00.000Z",
    consumedAt: "2026-04-22T01:00:00.000Z",
  });

  it("returns every record, newest first, by default", async () => {
    const store = mockLaunchStore([open1, open2, consumed]);
    const result = await listLaunches(store);
    expect(result.map((r) => r.id)).toEqual(["gamma-owl-3", "beta-bear-2", "alpha-fox-1"]);
  });

  it("filters to open packets when openOnly is set", async () => {
    const store = mockLaunchStore([open1, open2, consumed]);
    const result = await listLaunches(store, { openOnly: true });
    expect(result.map((r) => r.id)).toEqual(["beta-bear-2", "alpha-fox-1"]);
  });

  it("returns an empty array when no records exist", async () => {
    const store = mockLaunchStore([]);
    const result = await listLaunches(store);
    expect(result).toEqual([]);
  });
});
