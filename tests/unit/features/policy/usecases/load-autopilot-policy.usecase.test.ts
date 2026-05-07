import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadAutopilotPolicy } from "@/features/policy/usecases/load-autopilot-policy.usecase.js";
import { MaestroError } from "@/shared/errors.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-autopilot-policy-"));
  await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("loadAutopilotPolicy", () => {
  it("returns disabled defaults when file is missing", async () => {
    const policy = await loadAutopilotPolicy(tmpDir);

    expect(policy.kind).toBe("autopilot");
    expect(policy.autoMergeAllowed.low).toBe(false);
    expect(policy.autoMergeAllowed.medium).toBe(false);
    expect(policy.autoMergeAllowed.high).toBe(false);
    expect(policy.autoMergeAllowed.critical).toBe(false);
    expect(policy.requiredWitnessLevel.low).toBe("witnessed-by-maestro");
    expect(policy.requiredWitnessLevel.medium).toBe("witnessed-by-maestro");
    expect(policy.requiredWitnessLevel.high).toBe("witnessed-by-maestro");
    expect(policy.requiredWitnessLevel.critical).toBe("witnessed-by-maestro");
  });

  it("parses a valid autopilot.yaml", async () => {
    const fixture = join(
      import.meta.dir,
      "../../../../fixtures/policies/autopilot-valid.yaml",
    );
    const content = await readFile(fixture, "utf8");
    await writeFile(join(tmpDir, ".maestro", "policies", "autopilot.yaml"), content);

    const policy = await loadAutopilotPolicy(tmpDir);

    expect(policy.kind).toBe("autopilot");
    expect(policy.id).toBe("autopilot-policy-custom");
    expect(policy.autoMergeAllowed.low).toBe(true);
    expect(policy.autoMergeAllowed.medium).toBe(false);
    expect(policy.requiredWitnessLevel.low).toBe("witnessed-by-ci");
    expect(policy.requiredWitnessLevel.medium).toBe("witnessed-by-maestro");
  });

  it("throws MaestroError on unknown class key in auto_merge_allowed", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "autopilot.yaml"),
      "auto_merge_allowed:\n  low: true\n  ultra: true\n",
    );

    const err = await loadAutopilotPolicy(tmpDir).catch((e) => e);
    expect(err).toBeInstanceOf(MaestroError);
    expect(err.message).toMatch(/unknown risk class key/i);
  });

  it("throws MaestroError on unknown class key in required_witness_level", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "autopilot.yaml"),
      "required_witness_level:\n  low: witnessed-by-maestro\n  ultra: witnessed-by-maestro\n",
    );

    const err = await loadAutopilotPolicy(tmpDir).catch((e) => e);
    expect(err).toBeInstanceOf(MaestroError);
    expect(err.message).toMatch(/unknown risk class key/i);
  });

  it("throws MaestroError on invalid witness level value", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "autopilot.yaml"),
      "required_witness_level:\n  low: super-witnessed\n",
    );

    await expect(loadAutopilotPolicy(tmpDir)).rejects.toBeInstanceOf(MaestroError);
  });
});
