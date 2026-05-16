import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkStatus } from "@/infra/usecases/check-status.usecase.js";
import { mockConfig, mockGit } from "../../../helpers/mocks.js";

let cwd: string;
let homeDir: string;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-status-"));
  homeDir = join(cwd, "home");
  await mkdir(homeDir, { recursive: true });
});

afterEach(async () => {
  await rm(cwd, { recursive: true, force: true });
});

describe("checkStatus", () => {
  it("reports basic initialization state without pending handoff summaries", async () => {
    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      cwd,
      { homeDir },
    );

    expect(status).toEqual({
      initialized: true,
      configSource: "project",
      gitAvailable: true,
      legacyHandoffCount: 0,
    });
  });

  it("reports legacy launch artifacts in the project dir, ignoring current handoffs and home-dir launches", async () => {
    // Current handoffs are not legacy; canonical emit path is .maestro/handoffs/.
    const currentHandoffsDir = join(cwd, ".maestro", "handoffs");
    await mkdir(currentHandoffsDir, { recursive: true });
    await writeFile(join(currentHandoffsDir, "2026-04-20-001.json"), "{}\n");
    await writeFile(join(currentHandoffsDir, "2026-04-20-002.json"), "{}\n");
    const launchDir = join(cwd, ".maestro", "launches");
    await mkdir(launchDir, { recursive: true });
    await writeFile(join(launchDir, "2026-04-20-003.json"), "{}\n");
    // Home-dir launches belong to other repos and must not bleed into this
    // project's status output.
    const globalLaunchDir = join(homeDir, ".maestro", "launches");
    await mkdir(globalLaunchDir, { recursive: true });
    await writeFile(join(globalLaunchDir, "2026-04-20-004.json"), "{}\n");

    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      cwd,
      { homeDir },
    );

    expect(status.legacyHandoffCount).toBe(1);
  });
});
