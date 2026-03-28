import { describe, expect, it } from "bun:test";
import { runDoctor } from "../../../src/usecases/run-doctor.usecase.js";
import { mockGit, mockCass, mockConfig } from "../../helpers/mocks.js";

describe("runDoctor", () => {
  it("reports all checks as ok when everything is available", async () => {
    const config = mockConfig({
      exists: async () => true,
    });
    const checks = await runDoctor(mockCass(), mockGit(), config, process.cwd());

    expect(checks.length).toBe(4);
    expect(checks.every((c) => c.status === "ok")).toBe(true);
  });

  it("reports git as fail when not in repo", async () => {
    const git = mockGit({ isRepo: async () => false });
    const checks = await runDoctor(mockCass(), git, mockConfig(), process.cwd());

    const gitCheck = checks.find((c) => c.name === "git");
    expect(gitCheck?.status).toBe("fail");
    expect(gitCheck?.fix).toBeTruthy();
  });

  it("reports cass as fail when unavailable", async () => {
    const cass = mockCass({ isAvailable: async () => false });
    const checks = await runDoctor(cass, mockGit(), mockConfig(), process.cwd());

    const cassCheck = checks.find((c) => c.name === "cass");
    expect(cassCheck?.status).toBe("fail");
    expect(cassCheck?.fix).toContain("brew install");
  });

  it("reports missing config as warn with fix hint", async () => {
    const config = mockConfig({ exists: async () => false });
    const checks = await runDoctor(mockCass(), mockGit(), config, process.cwd());

    const projectCheck = checks.find((c) => c.name === "project-config");
    expect(projectCheck?.status).toBe("warn");
    expect(projectCheck?.fix).toContain("maestro init");
  });
});
