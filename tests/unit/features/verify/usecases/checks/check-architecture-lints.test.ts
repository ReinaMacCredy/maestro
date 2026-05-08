import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  checkArchitectureRules,
  checkArchitectureLints,
  isArchitectureRuleId,
  type ArchitectureViolation,
} from "@/features/verify/usecases/checks/check-architecture-lints.js";

let repoRoot: string;

beforeEach(async () => {
  repoRoot = await mkdtemp(join(tmpdir(), "arch-lints-"));
  await mkdir(join(repoRoot, "src"), { recursive: true });
});

afterEach(async () => {
  await rm(repoRoot, { recursive: true, force: true });
});

async function writeSrc(rel: string, body: string): Promise<void> {
  const abs = join(repoRoot, rel);
  await mkdir(join(abs, ".."), { recursive: true });
  await writeFile(abs, body);
}

describe("checkArchitectureRules — clean repo", () => {
  it("returns no error-severity violations on an empty src tree", async () => {
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.severity === "error")).toEqual([]);
  });
});

describe("no-runner-inversion", () => {
  it("triggers on Bun.spawn(['claude', ...])", async () => {
    await writeSrc(
      "src/foo.ts",
      `export async function spawnIt() {
  return Bun.spawn(["claude", "--help"]);
}`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    const hits = violations.filter((v) => v.ruleId === "no-runner-inversion");
    expect(hits.length).toBe(1);
    expect(hits[0]!.severity).toBe("error");
    expect(hits[0]!.file).toBe("src/foo.ts");
  });

  it("triggers on child_process.execFile('codex', ...)", async () => {
    await writeSrc(
      "src/bar.ts",
      `import * as child_process from "node:child_process";
child_process.execFile("codex", ["status"], () => {});`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    const hits = violations.filter((v) => v.ruleId === "no-runner-inversion");
    expect(hits.length).toBe(1);
  });

  it("respects // lint-arch-allow: no-runner-inversion comment", async () => {
    await writeSrc(
      "src/foo.ts",
      `export async function spawnIt() {
  // lint-arch-allow: no-runner-inversion
  return Bun.spawn(["claude", "--help"]);
}`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "no-runner-inversion")).toEqual([]);
  });

  it("does not trigger on tests/ or scripts/ files", async () => {
    await mkdir(join(repoRoot, "tests"), { recursive: true });
    await mkdir(join(repoRoot, "scripts"), { recursive: true });
    await writeFile(
      join(repoRoot, "tests/integration.ts"),
      `Bun.spawn(["claude"]);`,
    );
    await writeFile(
      join(repoRoot, "scripts/spawn.ts"),
      `Bun.spawn(["codex"]);`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "no-runner-inversion")).toEqual([]);
  });

  it("does not trigger on benign spawns (bun, git, etc.)", async () => {
    await writeSrc(
      "src/baz.ts",
      `Bun.spawn(["bun", "test"]);
Bun.spawn(["git", "diff"]);`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "no-runner-inversion")).toEqual([]);
  });
});

describe("single-opentui-render", () => {
  it("passes for a file with one root.render(", async () => {
    await writeSrc(
      "src/tui/app/render.tsx",
      `export function start() { root.render(<App />); }`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "single-opentui-render")).toEqual([]);
  });

  it("excludes **/testing/** even with multiple renders", async () => {
    await writeSrc(
      "src/tui/opentui/testing/frame-capture.tsx",
      `function a() { root.render(<A />); }
function b() { root.render(<B />); }`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "single-opentui-render")).toEqual([]);
  });

  it("triggers on a file with two real root.render calls", async () => {
    await writeSrc(
      "src/tui/app/render.tsx",
      `function a() { root.render(<A />); }
function b() { root.render(<B />); }`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    const hits = violations.filter((v) => v.ruleId === "single-opentui-render");
    expect(hits.length).toBeGreaterThanOrEqual(1);
    expect(hits[0]!.severity).toBe("error");
  });

  it("treats commented-out root.render as not a call", async () => {
    await writeSrc(
      "src/tui/app/render.tsx",
      `function a() { root.render(<A />); }
// in a comment: root.render(<B />)`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "single-opentui-render")).toEqual([]);
  });
});

describe("mission-control-readonly", () => {
  it("warns on a snapshot.ts function with await store.append()", async () => {
    await writeSrc(
      "src/tui/state/snapshot.ts",
      `export async function buildSnapshot(deps: any) {
  await deps.store.append({ row: 1 });
  return {};
}`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    const hits = violations.filter((v) => v.ruleId === "mission-control-readonly");
    expect(hits.length).toBe(1);
    expect(hits[0]!.severity).toBe("warn");
  });

  it("does not trigger when no write-shaped call is present", async () => {
    await writeSrc(
      "src/tui/state/snapshot.ts",
      `export async function buildSnapshot(deps: any) {
  const data = await deps.store.list();
  return data;
}`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    expect(violations.filter((v) => v.ruleId === "mission-control-readonly")).toEqual([]);
  });
});

describe("no-hand-edit-generated", () => {
  it("emits info-severity skip notice when no diff is supplied", async () => {
    const violations = await checkArchitectureRules({ repoRoot });
    const hits = violations.filter((v) => v.ruleId === "no-hand-edit-generated");
    expect(hits.length).toBe(1);
    expect(hits[0]!.severity).toBe("info");
  });

  it("emits no error when generated template is in diff WITH skills source", async () => {
    const violations = await checkArchitectureRules({
      repoRoot,
      diff: {
        base: "main",
        changedPaths: [
          "src/infra/domain/bundled-skill-templates.ts",
          "skills/bundled/maestro-verify/SKILL.md",
        ],
      },
    });
    expect(violations.filter((v) => v.ruleId === "no-hand-edit-generated" && v.severity === "error")).toEqual([]);
  });

  it("triggers error when bundled-skill-templates.ts is touched without skills/bundled/**", async () => {
    const violations = await checkArchitectureRules({
      repoRoot,
      diff: {
        base: "main",
        changedPaths: ["src/infra/domain/bundled-skill-templates.ts"],
      },
    });
    const hits = violations.filter((v) => v.ruleId === "no-hand-edit-generated");
    expect(hits.length).toBe(1);
    expect(hits[0]!.severity).toBe("error");
    expect(hits[0]!.file).toBe("src/infra/domain/bundled-skill-templates.ts");
  });

  it("triggers error when built-in-skill-templates.ts is touched alone", async () => {
    const violations = await checkArchitectureRules({
      repoRoot,
      diff: {
        base: "main",
        changedPaths: ["src/infra/domain/built-in-skill-templates.ts"],
      },
    });
    expect(violations.filter((v) => v.ruleId === "no-hand-edit-generated" && v.severity === "error").length).toBe(1);
  });
});

describe("checkArchitectureLints (Trust Verifier wrapper)", () => {
  it("returns TrustFinding[] with the correct check id and severity", async () => {
    await writeSrc(
      "src/danger.ts",
      `Bun.spawn(["claude", "--help"]);`,
    );
    const findings = await checkArchitectureLints(
      { base: "main", changedPaths: [] },
      repoRoot,
    );
    const errorFindings = findings.filter((f) => f.severity === "error");
    expect(errorFindings.length).toBeGreaterThanOrEqual(1);
    expect(errorFindings[0]!.check).toBe("no-runner-inversion");
    expect(errorFindings[0]!.paths).toContain("src/danger.ts");
    expect(errorFindings[0]!.details).toContain("Forbidden subprocess spawn");
    expect(errorFindings[0]!.details).toContain("Maestro must not spawn");
  });
});

describe("isArchitectureRuleId", () => {
  it("recognizes all four rule IDs", () => {
    expect(isArchitectureRuleId("no-runner-inversion")).toBe(true);
    expect(isArchitectureRuleId("single-opentui-render")).toBe(true);
    expect(isArchitectureRuleId("mission-control-readonly")).toBe(true);
    expect(isArchitectureRuleId("no-hand-edit-generated")).toBe(true);
  });

  it("rejects unrelated check IDs", () => {
    expect(isArchitectureRuleId("scope")).toBe(false);
    expect(isArchitectureRuleId("lockfile-parity")).toBe(false);
    expect(isArchitectureRuleId("")).toBe(false);
  });
});

describe("violation shape", () => {
  it("includes line and snippet for source-scanned rules", async () => {
    await writeSrc(
      "src/foo.ts",
      `// preamble
const x = 1;
Bun.spawn(["claude"]);`,
    );
    const violations = await checkArchitectureRules({ repoRoot });
    const hit = violations.find(
      (v: ArchitectureViolation) => v.ruleId === "no-runner-inversion",
    );
    expect(hit).toBeDefined();
    expect(typeof hit!.line).toBe("number");
    expect(hit!.snippet).toContain("Bun.spawn");
  });
});
