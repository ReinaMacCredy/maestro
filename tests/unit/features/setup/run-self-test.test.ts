import { describe, expect, it } from "bun:test";
import { runSetupSelfTest } from "@/features/setup";

describe("runSetupSelfTest", () => {
  it("runs a sandboxed self-test and returns ok=true on a clean run", async () => {
    const report = await runSetupSelfTest();
    expect(report.ok).toBe(true);
    expect(report.steps.length).toBeGreaterThan(0);
    expect(report.steps.find((s) => s.name === "evidence-store")?.ok).toBe(true);
  });

  it("reports steps with names", async () => {
    const report = await runSetupSelfTest();
    const names = report.steps.map((s) => s.name);
    expect(names).toContain("layout");
    expect(names).toContain("evidence-store");
  });
});
