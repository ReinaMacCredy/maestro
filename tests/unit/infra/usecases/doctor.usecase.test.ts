import { describe, expect, it } from "bun:test";
import { runDoctor } from "@/usecases/run-doctor.usecase.js";
import { mockGit, mockConfig } from "../../helpers/mocks.js";

describe("runDoctor", () => {
  it("reports all checks as ok when everything is available", async () => {
    const config = mockConfig({
      exists: async () => true,
    });
    const checks = await runDoctor(mockGit(), config, process.cwd());

    expect(checks.length).toBe(3);
    expect(checks.every((c) => c.status === "ok")).toBe(true);
  });

  it("reports git as fail when not in repo", async () => {
    const git = mockGit({ isRepo: async () => false });
    const checks = await runDoctor(git, mockConfig(), process.cwd());

    const gitCheck = checks.find((c) => c.name === "git");
    expect(gitCheck?.status).toBe("fail");
    expect(gitCheck?.fix).toBeTruthy();
  });

  it("reports missing config as warn with fix hint", async () => {
    const config = mockConfig({ exists: async () => false });
    const checks = await runDoctor(mockGit(), config, process.cwd());

    const projectCheck = checks.find((c) => c.name === "project-config");
    expect(projectCheck?.status).toBe("warn");
    expect(projectCheck?.fix).toContain("maestro init");
  });

  it("warns when a global-only mission control setting is set in project config", async () => {
    const config = mockConfig({
      exists: async () => true,
      loadLayers: async () => ({
        defaults: {
          ui: {
            missionControl: {
              backgroundMode: "solid",
            },
          },
        },
        effective: {
          ui: {
            missionControl: {
              backgroundMode: "terminal",
            },
          },
        },
        global: {
          ui: {
            missionControl: {
              backgroundMode: "terminal",
            },
          },
        },
        project: {
          ui: {
            missionControl: {
              backgroundMode: "solid",
            },
          },
        },
        errors: [],
        paths: {
          project: ".maestro/config.yaml",
          global: "~/.maestro/config.yaml",
        },
      }),
    });

    const checks = await runDoctor(mockGit(), config, process.cwd());

    expect(checks.find((c) => c.name === "ignored-ui-missionControl-backgroundMode")).toMatchObject({
      status: "warn",
    });
  });
});
