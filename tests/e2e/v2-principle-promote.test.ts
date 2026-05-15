import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-principle-promote-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function seedLintViolation(dir: string, row: object): Promise<void> {
  const day = new Date().toISOString().slice(0, 10);
  const evidenceDir = join(dir, ".maestro/evidence");
  await mkdir(evidenceDir, { recursive: true });
  await writeFile(join(evidenceDir, `${day}.jsonl`), `${JSON.stringify(row)}\n`, "utf8");
}

describe("maestro principle promote (v2)", () => {
  it("writes docs/principles/<slug>.md from a lint-violation row", async () => {
    await seedLintViolation(tmpDir, {
      id: "evd-test-001",
      kind: "lint-violation",
      timestamp: "2026-05-15T10:00:00Z",
      rule_id: "prefer_shared_utils",
      severity: "error",
      file: "src/x.ts",
      line: 12,
      message: "duplicate helper",
      remediation: "Move to src/shared/lib.",
    });

    const result = await runCompiled(
      ["principle", "promote", "evd-test-001"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("prefer-shared-utils -> docs/principles/prefer-shared-utils.md");
    expect(result.stdout).toContain("rule_id=prefer_shared_utils");

    const content = await readFile(
      join(tmpDir, "docs/principles/prefer-shared-utils.md"),
      "utf8",
    );
    expect(content).toContain("# prefer-shared-utils");
    expect(content).toContain("## Rule");
    expect(content).toContain("duplicate helper");
    expect(content).toContain("## Rationale");
    expect(content).toContain("evd-test-001");
    expect(content).toContain("src/x.ts:12");
    expect(content).toContain("## Scan Command");
    expect(content).toContain("## Fix Recipe");
    expect(content).toContain("Move to src/shared/lib");
  });

  it("collision-suffix when slug already exists", async () => {
    await seedLintViolation(tmpDir, {
      id: "evd-test-002",
      kind: "lint-violation",
      timestamp: "2026-05-15T10:01:00Z",
      rule_id: "layer_order",
      severity: "error",
      file: "src/v2/repo/x.ts",
      line: 1,
      message: "violation",
    });
    const principlesDir = join(tmpDir, "docs/principles");
    await mkdir(principlesDir, { recursive: true });
    await writeFile(join(principlesDir, "layer-order.md"), "existing\n", "utf8");

    const result = await runCompiled(
      ["principle", "promote", "evd-test-002"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("layer-order-2 -> docs/principles/layer-order-2.md");
  });

  it("exits 1 with CorrectionNotFoundError when id is unknown", async () => {
    await seedLintViolation(tmpDir, {
      id: "evd-real",
      kind: "lint-violation",
      timestamp: "2026-05-15T10:02:00Z",
      rule_id: "x",
      severity: "error",
      file: "f.ts",
      message: "m",
    });
    const result = await runCompiled(
      ["principle", "promote", "evd-not-real"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("not found");
  });

  it("--json emits machine-readable result", async () => {
    await seedLintViolation(tmpDir, {
      id: "evd-test-003",
      kind: "lint-violation",
      timestamp: "2026-05-15T10:03:00Z",
      rule_id: "no_yolo_data_probing",
      severity: "error",
      file: "src/y.ts",
      line: 7,
      message: "shell read into JSONL",
    });
    const result = await runCompiled(
      ["principle", "promote", "evd-test-003", "--json"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    const parsed = JSON.parse(result.stdout) as {
      slug: string;
      path: string;
      rule_id: string;
      correction_id: string;
    };
    expect(parsed.slug).toBe("no-yolo-data-probing");
    expect(parsed.path).toBe("docs/principles/no-yolo-data-probing.md");
    expect(parsed.rule_id).toBe("no_yolo_data_probing");
    expect(parsed.correction_id).toBe("evd-test-003");
  });

  it("refuses to promote a transition evidence row", async () => {
    await seedLintViolation(tmpDir, {
      id: "evd-trans-001",
      kind: "transition",
      timestamp: "2026-05-15T10:04:00Z",
      from_state: null,
      to_state: "draft",
      trigger_verb: "task:from-spec",
    });
    const result = await runCompiled(
      ["principle", "promote", "evd-trans-001"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("only lint-violation");
  });
});
