import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkStatus } from "@/infra/usecases/check-status.usecase.js";
import { mockConfig, mockGit } from "../../../helpers/mocks.js";

let cwd: string;

beforeEach(async () => {
  cwd = await mkdtemp(join(tmpdir(), "maestro-status-"));
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
    );

    expect(status).toEqual({
      initialized: true,
      configSource: "project",
      gitAvailable: true,
      legacyHandoffCount: 0,
    });
  });

  it("reports legacy handoff artifacts when .maestro/handoffs exists", async () => {
    const legacyDir = join(cwd, ".maestro", "handoffs");
    await mkdir(legacyDir, { recursive: true });
    await writeFile(join(legacyDir, "2026-04-20-001.json"), "{}\n");
    await writeFile(join(legacyDir, "2026-04-20-002.json"), "{}\n");

    const status = await checkStatus(
      mockConfig({ exists: async () => true }),
      mockGit(),
      cwd,
    );

    expect(status.legacyHandoffCount).toBe(2);
  });
});
