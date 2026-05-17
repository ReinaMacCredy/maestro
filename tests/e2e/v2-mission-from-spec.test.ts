import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
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
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-v2-plan-from-spec-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function readEvidenceRows(dir: string): Promise<readonly Record<string, unknown>[]> {
  const evidenceDir = join(dir, ".maestro/evidence");
  let entries: string[];
  try {
    entries = await readdir(evidenceDir);
  } catch {
    return [];
  }
  const files = entries.filter((f) => f.endsWith(".jsonl")).sort();
  const rows: Record<string, unknown>[] = [];
  for (const f of files) {
    const text = await readFile(join(evidenceDir, f), "utf8");
    for (const line of text.split("\n")) {
      if (line.length === 0) continue;
      rows.push(JSON.parse(line));
    }
  }
  return rows;
}

describe("maestro mission from-spec + mission show (v2)", () => {
  it("mission from-spec on a heavy-mode spec creates a mission in 'specified' and emits one transition row", async () => {
    await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], tmpDir);
    const result = await runCompiled(
      ["mission", "from-spec", ".maestro/specs/demo-heavy.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toMatch(/^pln-\S+ specified \(demo-heavy\)$/m);

    const text = await readFile(join(tmpDir, ".maestro/missions/plans.jsonl"), "utf8");
    const lines = text.trim().split("\n");
    expect(lines.length).toBe(1);
    const mission = JSON.parse(lines[0]) as { state: string; slug: string };
    expect(mission.state).toBe("specified");
    expect(mission.slug).toBe("demo-heavy");

    const rows = await readEvidenceRows(tmpDir);
    expect(rows.length).toBe(1);
    expect(rows[0]).toMatchObject({
      kind: "transition",
      from_state: null,
      to_state: "specified",
      trigger_verb: "mission:from-spec",
    });
  });

  it("mission from-spec on a light-mode spec fails with MissionRequiresHeavyModeError", async () => {
    await runCompiled(["spec", "new", "demo-light"], tmpDir);
    const result = await runCompiled(
      ["mission", "from-spec", ".maestro/specs/demo-light.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("mission from-spec requires mode: heavy");
  });

  it("mission show emits text by default and JSON with --json", async () => {
    await runCompiled(["spec", "new", "demo-heavy", "--mode", "heavy"], tmpDir);
    const created = await runCompiled(
      ["mission", "from-spec", ".maestro/specs/demo-heavy.md"],
      tmpDir,
    );
    const missionId = (created.stdout.match(/^(pln-\S+)/) ?? [])[1];
    expect(missionId).toBeDefined();

    const text = await runCompiled(["mission", "show", missionId!], tmpDir);
    expect(text.exitCode).toBe(0);
    expect(text.stdout).toContain(`${missionId} specified`);
    expect(text.stdout).toContain("(no child tasks yet)");

    const json = await runCompiled(["mission", "show", missionId!, "--json"], tmpDir);
    expect(json.exitCode).toBe(0);
    const parsed = JSON.parse(json.stdout) as { mission: { id: string; state: string }; tasks: unknown[] };
    expect(parsed.mission.id).toBe(missionId!);
    expect(parsed.mission.state).toBe("specified");
    expect(parsed.tasks).toEqual([]);
  });

  it("mission show on a missing id fails with MissionNotFoundError", async () => {
    const result = await runCompiled(["mission", "show", "pln-doesnotexist"], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("not found");
  });
});
