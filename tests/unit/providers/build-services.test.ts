import { describe, expect, it } from "bun:test";
import { buildCoreServices } from "@/providers/build-services.js";

describe("buildCoreServices", () => {
  it("wires every default port including principlesStore and processRunner", () => {
    const svc = buildCoreServices({ repoRoot: "/tmp" });
    expect(svc.specStore).toBeDefined();
    expect(svc.taskStore).toBeDefined();
    expect(svc.missionStore).toBeDefined();
    expect(svc.evidenceStore).toBeDefined();
    expect(svc.architectureRules).toBeDefined();
    expect(svc.principlesStore).toBeDefined();
    expect(svc.processRunner).toBeDefined();
    expect(svc.nowMdWriter).toBeDefined();
  });

  it("respects overrides for new ports", () => {
    const fakeRunner = { run: async () => ({ stdout: "", stderr: "", exitCode: 0 }) };
    const svc = buildCoreServices({
      repoRoot: "/tmp",
      overrides: { processRunner: fakeRunner },
    });
    expect(svc.processRunner).toBe(fakeRunner);
  });
});
