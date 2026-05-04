import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { detectPendingLoosenings } from "@/features/policy/usecases/detect-pending-loosenings.usecase.js";

let tmpDir: string;

function git(args: string[], cwd: string): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" });
}

async function setupGitRepo(dir: string): Promise<void> {
  git(["init", "-b", "main"], dir);
  git(["config", "user.email", "test@example.com"], dir);
  git(["config", "user.name", "Test"], dir);
  git(["config", "commit.gpgsign", "false"], dir);
  await mkdir(join(dir, ".maestro", "policies"), { recursive: true });
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-detect-pending-"));
  await setupGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

// All witness levels at the maximum — going from empty (all default to witnessed-by-maestro)
// to this file only adds autoMergeAllowed.* = false (no-op) and keeps all witness levels
// at witnessed-by-maestro, so no loosenings occur.
const TIGHTENING_YAML = `
auto_merge_allowed:
  low: false
  medium: false
  high: false
  critical: false
required_witness_level:
  low: witnessed-by-maestro
  medium: witnessed-by-maestro
  high: witnessed-by-maestro
  critical: witnessed-by-maestro
`.trim();

// loosening: high dropped from witnessed-by-maestro to agent-claimed-locally
const LOOSENING_YAML = `
auto_merge_allowed:
  low: false
  medium: false
  high: false
  critical: false
required_witness_level:
  low: witnessed-by-maestro
  medium: witnessed-by-maestro
  high: agent-claimed-locally
  critical: witnessed-by-maestro
`.trim();

const AUTOPILOT_PATH = ".maestro/policies/autopilot.yaml";

describe("detectPendingLoosenings", () => {
  it("tightening commit does not appear in pending loosenings", async () => {
    // Commit 1: write the tightening YAML
    await writeFile(join(tmpDir, AUTOPILOT_PATH), TIGHTENING_YAML, "utf8");
    git(["add", AUTOPILOT_PATH], tmpDir);
    git(["commit", "-m", "tightening: raise witness for high"], tmpDir);

    const loosenings = await detectPendingLoosenings({ projectRoot: tmpDir });
    expect(loosenings).toHaveLength(0);
  });

  it("loosening commit appears in pending loosenings with correct effectiveAt", async () => {
    // Commit 1: the tightening baseline
    await writeFile(join(tmpDir, AUTOPILOT_PATH), TIGHTENING_YAML, "utf8");
    git(["add", AUTOPILOT_PATH], tmpDir);
    git(["commit", "-m", "baseline: tightening"], tmpDir);

    // Commit 2: a loosening (lower high from witnessed-by-maestro to agent-claimed-locally)
    await writeFile(join(tmpDir, AUTOPILOT_PATH), LOOSENING_YAML, "utf8");
    git(["add", AUTOPILOT_PATH], tmpDir);
    git(["commit", "-m", "loosening: lower witness for high"], tmpDir);

    const loosenings = await detectPendingLoosenings({ projectRoot: tmpDir });

    // Should have exactly one loosening: the requiredWitnessLevel.high change
    const witnessLoosenings = loosenings.filter(
      (l) => l.edit.path === "requiredWitnessLevel.high",
    );
    expect(witnessLoosenings.length).toBeGreaterThanOrEqual(1);

    const l = witnessLoosenings[0];
    expect(l.kind).toBe("autopilot");
    expect(l.file).toBe(AUTOPILOT_PATH);
    expect(l.edit.oldValue).toBe("witnessed-by-maestro");
    expect(l.edit.newValue).toBe("agent-claimed-locally");

    // effectiveAt should be ~30 days from now (committed just now)
    const effectiveAt = Date.parse(l.effectiveAt);
    const now = Date.now();
    const diffDays = (effectiveAt - now) / (1000 * 60 * 60 * 24);
    // Within 28–32 days (rounding tolerance)
    expect(diffDays).toBeGreaterThan(28);
    expect(diffDays).toBeLessThan(32);
  });

  it("returns empty array in a repo with no policy history", async () => {
    // No commits, just an initial empty repo
    const loosenings = await detectPendingLoosenings({ projectRoot: tmpDir });
    expect(loosenings).toHaveLength(0);
  });
});
