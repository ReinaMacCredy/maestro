import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runCli } from "../../../helpers/run-cli.js";

let tmpDir: string;

describe("handoff status integration", () => {
  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "maestro-handoff-status-"));
  });

  afterEach(async () => {
    await rm(tmpDir, { recursive: true, force: true });
  });

  it("status --json reports pending UKI handoffs", async () => {
    const create = await runCli([
      "handoff",
      "create",
      "--mode", "execute",
      "--session-core", "status_test",
      "--summary", "Status_test-visible-low_risk",
      "--next-action", "inspect_status",
      "--decision", "keep_status_visible",
      "--validation", "status_green",
      "--artifact", "branch_main",
      "--read-more", "branch_main",
      "--completed", "status_handoff_created",
      "--confidence-work", "0.9",
      "--json",
    ], tmpDir);
    expect(create.exitCode).toBe(0);

    const status = await runCli(["status", "--json"], tmpDir);
    expect(status.exitCode).toBe(0);
    const parsed = JSON.parse(status.stdout);
    expect(parsed.pendingHandoffs).toHaveLength(1);
  });
});
