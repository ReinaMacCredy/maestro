import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadOwners } from "@/features/policy/usecases/load-owners.usecase.js";
import { MaestroError } from "@/shared/errors.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-policy-"));
  await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("loadOwners", () => {
  it("parses empty-roles scaffold to empty arrays", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "owners.yaml"),
      "policy_approver: []\nratchet_approver: []\nsensitive_waiver: []\n",
    );

    const owners = await loadOwners(tmpDir);

    expect(owners.policyApprovers).toEqual([]);
    expect(owners.ratchetApprovers).toEqual([]);
    expect(owners.sensitiveWaivers).toEqual([]);
  });

  it("parses populated roles correctly", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "owners.yaml"),
      'policy_approver: ["@alice", "@org/team"]\nratchet_approver: ["@bob"]\nsensitive_waiver: []\n',
    );

    const owners = await loadOwners(tmpDir);

    expect(owners.policyApprovers).toEqual(["@alice", "@org/team"]);
    expect(owners.ratchetApprovers).toEqual(["@bob"]);
    expect(owners.sensitiveWaivers).toEqual([]);
  });

  it("omitted roles default to empty arrays", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "owners.yaml"),
      "policy_approver: [\"@alice\"]\n",
    );

    const owners = await loadOwners(tmpDir);

    expect(owners.policyApprovers).toEqual(["@alice"]);
    expect(owners.ratchetApprovers).toEqual([]);
    expect(owners.sensitiveWaivers).toEqual([]);
  });

  it("throws MaestroError /malformed/i on invalid YAML", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "owners.yaml"),
      "policy_approver: [\nunclosed bracket\n",
    );

    await expect(loadOwners(tmpDir)).rejects.toThrow(/malformed/i);
  });

  it("throws MaestroError /malformed/i when a role is not an array", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "owners.yaml"),
      "policy_approver: not-a-list\n",
    );

    await expect(loadOwners(tmpDir)).rejects.toThrow(/malformed/i);
  });

  it("throws MaestroError with MaestroError name on missing file", async () => {
    const err = await loadOwners(tmpDir).catch((e) => e);
    expect(err).toBeInstanceOf(MaestroError);
    expect(err.message).toMatch(/not found/i);
    expect(err.hints).toContain("Run 'maestro init' to scaffold it");
  });

  it("does not silently return empty owners when file is missing", async () => {
    await expect(loadOwners(tmpDir)).rejects.toThrow(MaestroError);
  });
});
