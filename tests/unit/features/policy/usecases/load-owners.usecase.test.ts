import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadOwners, loadOwnersFromBase, parseOwners } from "@/features/policy/usecases/load-owners.usecase.js";
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

describe("loadOwnersFromBase", () => {
  const EMPTY_TREE_SHA = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

  it("emits a tailored hint when base resolves to the empty tree", async () => {
    execFileSync("git", ["init", "-q"], { cwd: tmpDir });
    execFileSync("git", ["config", "user.email", "t@t.com"], { cwd: tmpDir });
    execFileSync("git", ["config", "user.name", "t"], { cwd: tmpDir });
    const err = await loadOwnersFromBase(EMPTY_TREE_SHA, tmpDir).catch((e) => e);
    expect(err).toBeInstanceOf(MaestroError);
    expect((err as MaestroError).message).toMatch(/empty-tree base/i);
    expect((err as MaestroError).hints.join("\n")).toMatch(/--base/);
  });
});

describe("parseOwners — deploy_approver", () => {
  it("populates deployApprovers from an array of usernames", () => {
    const owners = parseOwners(
      'policy_approver: []\nratchet_approver: []\nsensitive_waiver: []\ndeploy_approver: ["@alice", "@bob"]\n',
    );
    expect(owners.deployApprovers).toEqual(["@alice", "@bob"]);
  });

  it("coerces null deploy_approver to empty array", () => {
    const owners = parseOwners(
      "policy_approver: []\nratchet_approver: []\nsensitive_waiver: []\ndeploy_approver: null\n",
    );
    expect(owners.deployApprovers).toEqual([]);
  });

  it("defaults deployApprovers to empty array when field is missing", () => {
    const owners = parseOwners(
      "policy_approver: []\nratchet_approver: []\nsensitive_waiver: []\n",
    );
    expect(owners.deployApprovers).toEqual([]);
  });

  it("throws MaestroError /malformed/i when deploy_approver is not an array", () => {
    expect(() =>
      parseOwners(
        "policy_approver: []\nratchet_approver: []\nsensitive_waiver: []\ndeploy_approver: not-an-array\n",
      ),
    ).toThrow(/malformed/i);
  });
});
