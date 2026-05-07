import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runDoctor } from "@/infra/usecases/run-doctor.usecase.js";
import { mockGit, mockConfig } from "../../../helpers/mocks.js";

let cwd: string;
let homeDir: string;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-doctor-"));
  homeDir = join(cwd, "home");
  await mkdir(homeDir, { recursive: true });
});

afterEach(async () => {
  await rm(cwd, { recursive: true, force: true });
});

describe("runDoctor", () => {
  it("reports all checks as ok when everything is available", async () => {
    const config = mockConfig({
      exists: async () => true,
    });
    const checks = await runDoctor(mockGit(), config, cwd, { homeDir });

    expect(checks.length).toBe(3);
    expect(checks.every((c) => c.status === "ok")).toBe(true);
  });

  it("reports git as fail when not in repo", async () => {
    const git = mockGit({ isRepo: async () => false });
    const checks = await runDoctor(git, mockConfig(), cwd, { homeDir });

    const gitCheck = checks.find((c) => c.name === "git");
    expect(gitCheck?.status).toBe("fail");
    expect(gitCheck?.fix).toBeTruthy();
  });

  it("reports missing config as warn with fix hint", async () => {
    const config = mockConfig({ exists: async () => false });
    const checks = await runDoctor(mockGit(), config, cwd, { homeDir });

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

    const checks = await runDoctor(mockGit(), config, cwd, { homeDir });

    expect(checks.find((c) => c.name === "ignored-ui-missionControl-backgroundMode")).toMatchObject({
      status: "warn",
    });
  });

  it("warns for each empty src/features/* subdirectory", async () => {
    await mkdir(join(cwd, "src", "features", "ghost", "commands"), { recursive: true });
    await mkdir(join(cwd, "src", "features", "ghost", "domain"), { recursive: true });
    await mkdir(join(cwd, "src", "features", "real"), { recursive: true });
    await writeFile(join(cwd, "src", "features", "real", "index.ts"), "export {};\n");

    const checks = await runDoctor(
      mockGit(),
      mockConfig({ exists: async () => true }),
      cwd,
      { homeDir },
    );

    const ghost = checks.find((c) => c.name === "empty-feature-ghost");
    expect(ghost).toMatchObject({ status: "warn" });
    expect(ghost?.message).toContain("ghost");
    expect(checks.find((c) => c.name === "empty-feature-real")).toBeUndefined();
  });

  it("stays silent when src/features/ does not exist", async () => {
    const checks = await runDoctor(
      mockGit(),
      mockConfig({ exists: async () => true }),
      cwd,
      { homeDir },
    );
    expect(checks.some((c) => c.name.startsWith("empty-feature-"))).toBe(false);
  });

  it("warns for oversized root markdown docs not in allowlist", async () => {
    const big = "a\n".repeat(700);
    await writeFile(join(cwd, "PROPOSAL.md"), big);
    await writeFile(join(cwd, "README.md"), big); // allowlisted
    await writeFile(join(cwd, "TINY.md"), "small\n");

    const checks = await runDoctor(
      mockGit(),
      mockConfig({ exists: async () => true }),
      cwd,
      { homeDir },
    );

    const proposal = checks.find((c) => c.name === "oversized-root-doc-PROPOSAL-md");
    expect(proposal).toMatchObject({ status: "warn" });
    expect(proposal?.message).toContain("701");
    expect(proposal?.fix).toContain("docs/proposals/");
    expect(checks.some((c) => c.name === "oversized-root-doc-README-md")).toBe(false);
    expect(checks.some((c) => c.name === "oversized-root-doc-TINY-md")).toBe(false);
  });

  it("warns when legacy handoff or launch artifacts are still present", async () => {
    await mkdir(join(cwd, ".maestro", "handoffs"), { recursive: true });
    await writeFile(join(cwd, ".maestro", "handoffs", "2026-04-20-001.json"), "{}\n");
    await mkdir(join(cwd, ".maestro", "launches"), { recursive: true });
    await writeFile(join(cwd, ".maestro", "launches", "2026-04-20-002.json"), "{}\n");
    await mkdir(join(homeDir, ".maestro", "launches"), { recursive: true });
    await writeFile(join(homeDir, ".maestro", "launches", "2026-04-20-003.json"), "{}\n");

    const checks = await runDoctor(
      mockGit(),
      mockConfig({ exists: async () => true }),
      cwd,
      { homeDir },
    );

    expect(checks.find((check) => check.name === "legacy-handoffs")).toMatchObject({
      status: "warn",
      message: "Found 3 legacy handoff artifact(s) under .maestro/handoffs/, .maestro/launches/, or ~/.maestro/launches/",
    });
  });
});
