import { describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { auditInstall } from "@/features/setup";

async function makeRepo(opts: {
  agentsMdLines?: number;
  withDocs?: boolean;
  withOwners?: boolean;
}): Promise<string> {
  const tmp = await mkdtemp(join(tmpdir(), "setup-audit-"));
  if (opts.agentsMdLines && opts.agentsMdLines > 0) {
    const content = Array.from({ length: opts.agentsMdLines }, (_, i) => `line ${i}`).join("\n");
    await writeFile(join(tmp, "AGENTS.md"), content + "\n", "utf8");
  }
  if (opts.withDocs) {
    await mkdir(join(tmp, "docs"), { recursive: true });
    await writeFile(join(tmp, "docs/harness-positioning.md"), "x\n", "utf8");
    await writeFile(join(tmp, "docs/schedule-recipes.md"), "x\n", "utf8");
  }
  if (opts.withOwners) {
    await mkdir(join(tmp, ".maestro/policies"), { recursive: true });
    await writeFile(
      join(tmp, ".maestro/policies/owners.yaml"),
      "policy_approver: [a]\nratchet_approver: [a]\nsensitive_waiver: [a]\ndeploy_approver: [a]\n",
      "utf8",
    );
  }
  return tmp;
}

describe("auditInstall", () => {
  it("warns when AGENTS.md is missing", async () => {
    const root = await makeRepo({});
    const r = await auditInstall({ projectRoot: root, knownVerbs: new Set() });
    expect(r.findings.some((f) => f.code === "agents-md-missing")).toBe(true);
  });

  it("errors when AGENTS.md is too large", async () => {
    const root = await makeRepo({ agentsMdLines: 200 });
    const r = await auditInstall({ projectRoot: root, knownVerbs: new Set() });
    expect(r.findings.some((f) => f.code === "agents-md-too-large")).toBe(true);
    expect(r.ok).toBe(false);
  });

  it("reports no errors on a tidy repo", async () => {
    const root = await makeRepo({ agentsMdLines: 80, withDocs: true, withOwners: true });
    const r = await auditInstall({ projectRoot: root, knownVerbs: new Set(["session", "evidence"]) });
    const errs = r.findings.filter((f) => f.severity === "error");
    expect(errs).toEqual([]);
  });

  it("does not flag fresh user projects for missing maestro-repo-internal docs", async () => {
    // docs/harness-positioning.md and docs/schedule-recipes.md live in the
    // maestro repo itself; a freshly init'd user project should not be warned
    // for their absence.
    const root = await makeRepo({ agentsMdLines: 50 });
    const r = await auditInstall({ projectRoot: root, knownVerbs: new Set() });
    expect(r.findings.some((f) => f.code === "doc-missing")).toBe(false);
  });
});
