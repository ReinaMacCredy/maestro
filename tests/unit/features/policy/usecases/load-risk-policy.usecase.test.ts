import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadRiskPolicy } from "@/features/policy/usecases/load-risk-policy.usecase.js";
import { DEFAULT_RISK_POLICY } from "@/features/policy/domain/risk-policy-defaults.js";
import { MaestroError } from "@/shared/errors.js";
import { PROJECT_BOOTSTRAP_TEMPLATES } from "@/infra/domain/bootstrap-templates.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import type { RiskPolicy } from "@/features/policy/index.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-risk-policy-"));
  await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("loadRiskPolicy", () => {
  it("returns DEFAULT_RISK_POLICY when file is missing", async () => {
    const policy = await loadRiskPolicy(tmpDir);
    expect(policy).toEqual(DEFAULT_RISK_POLICY);
  });

  it("parses a valid custom risk.yaml", async () => {
    const fixture = join(
      import.meta.dir,
      "../../../../fixtures/policies/risk-valid.yaml",
    );
    const content = await readFile(fixture, "utf8");
    await writeFile(join(tmpDir, ".maestro", "policies", "risk.yaml"), content);

    const policy = await loadRiskPolicy(tmpDir);

    expect(policy.kind).toBe("risk");
    expect(policy.id).toBe("risk-policy-custom");
    expect(policy.version).toBe("2");
    expect(policy.rows).toHaveLength(2);
    expect(policy.rows[0].signal).toBe("diff-intersects-sensitive-security");
    expect(policy.rows[0].derivedClass).toBe("critical");
    expect(policy.rows[1].signal).toBe("diff-source-only");
    expect(policy.rows[1].derivedClass).toBe("low");
  });

  it("throws MaestroError /risk.yaml malformed/ on malformed YAML", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "risk.yaml"),
      "rows:\n  - signal: [unclosed\n",
    );

    await expect(loadRiskPolicy(tmpDir)).rejects.toThrow(/risk\.yaml malformed/i);
    await expect(loadRiskPolicy(tmpDir)).rejects.toBeInstanceOf(MaestroError);
  });

  it("throws MaestroError /unknown derived_class/ for invalid derived_class value", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "risk.yaml"),
      "rows:\n  - signal: my-signal\n    derived_class: supersecure\n",
    );

    const err = await loadRiskPolicy(tmpDir).catch((e) => e);
    expect(err).toBeInstanceOf(MaestroError);
    expect(err.message).toMatch(/unknown derived_class/i);
  });

  it("parity: bootstrap template risk.yaml parses to DEFAULT_RISK_POLICY", async () => {
    const template = PROJECT_BOOTSTRAP_TEMPLATES.find(
      (t) => t.path === ".maestro/policies/risk.yaml",
    );
    expect(template).toBeDefined();

    await writeFile(
      join(tmpDir, ".maestro", "policies", "risk.yaml"),
      template!.content,
    );

    const policy = await loadRiskPolicy(tmpDir);

    // The template should parse to the same shape as DEFAULT_RISK_POLICY
    expect(policy.kind).toBe(DEFAULT_RISK_POLICY.kind);
    expect(policy.rows).toHaveLength(DEFAULT_RISK_POLICY.rows.length);

    for (let i = 0; i < DEFAULT_RISK_POLICY.rows.length; i++) {
      expect(policy.rows[i].signal).toBe(DEFAULT_RISK_POLICY.rows[i].signal);
      expect(policy.rows[i].derivedClass).toBe(DEFAULT_RISK_POLICY.rows[i].derivedClass);
    }
  });

  it("throws MaestroError when rows field is missing", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "risk.yaml"),
      "kind: risk\nid: test\n",
    );

    await expect(loadRiskPolicy(tmpDir)).rejects.toThrow(/malformed/i);
  });

  it("uses 'risk-policy-custom' id when id field is absent", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "risk.yaml"),
      "rows:\n  - signal: diff-source-only\n    derived_class: medium\n",
    );

    const policy = await loadRiskPolicy(tmpDir);
    expect(policy.id).toBe("risk-policy-custom");
  });
});
