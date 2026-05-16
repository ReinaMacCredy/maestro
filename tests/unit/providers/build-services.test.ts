import { describe, expect, it } from "bun:test";
import { buildV2Services } from "@/providers/build-services.js";

describe("buildV2Services", () => {
  it("wires every default port including principlesStore and processRunner", () => {
    const svc = buildV2Services({ repoRoot: "/tmp" });
    expect(svc.specStore).toBeDefined();
    expect(svc.taskStore).toBeDefined();
    expect(svc.planStore).toBeDefined();
    expect(svc.evidenceStore).toBeDefined();
    expect(svc.architectureRules).toBeDefined();
    expect(svc.principlesStore).toBeDefined();
    expect(svc.processRunner).toBeDefined();
    expect(svc.nowMdWriter).toBeDefined();
  });

  it("respects overrides for new ports", () => {
    const fakeRunner = { run: async () => ({ stdout: "", stderr: "", exitCode: 0 }) };
    const svc = buildV2Services({
      repoRoot: "/tmp",
      overrides: { processRunner: fakeRunner },
    });
    expect(svc.processRunner).toBe(fakeRunner);
  });
});
