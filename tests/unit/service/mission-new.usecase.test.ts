import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  missionNew,
  MissionNewInvalidFlagsError,
  MissionTemplateUnknownError,
  slugifyTitle,
} from "@/service/mission-new.usecase.js";
import { JsonlMissionStore } from "@/repo/jsonl-mission-store.adapter.js";
import { JsonlTaskStore } from "@/repo/jsonl-task-store.adapter.js";
import { JsonlEvidenceStore } from "@/repo/jsonl-evidence-store.adapter.js";
import { DuplicateMissionSlugError } from "@/repo/mission-store.port.js";

const HEAVY_SPEC = `---
slug: demo-plan
acceptance_criteria:
  - it works
non_goals:
  - nothing
risk_class: medium
mode: heavy
work_type: change-request
---

# Demo plan

A heavy-mode spec.
`;

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-mission-new-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

function services() {
  return {
    repoRoot: tmpDir,
    missionStore: new JsonlMissionStore({ repoRoot: tmpDir }),
    taskStore: new JsonlTaskStore({ repoRoot: tmpDir }),
    evidenceStore: new JsonlEvidenceStore({ repoRoot: tmpDir }),
  };
}

describe("missionNew", () => {
  it("bare title creates a mission at intake with no tasks", async () => {
    const result = await missionNew(services(), {
      title: "Try out a new approach",
      slug: "try-new-approach",
      mode: "bare",
    });
    expect(result.mission.state).toBe("intake");
    expect(result.mission.slug).toBe("try-new-approach");
    expect(result.tasks).toEqual([]);
  });

  it("--template refactor seeds tasks and lands at planned", async () => {
    const result = await missionNew(services(), {
      title: "Refactor auth",
      slug: "refactor-auth",
      mode: "template",
      template: "refactor",
    });
    expect(result.mission.state).toBe("planned");
    expect(result.tasks.length).toBe(4);
    // Slugs are namespaced under the mission slug so they're globally unique.
    expect(result.tasks.every((t) => t.slug.startsWith("refactor-auth-"))).toBe(true);
    expect(result.tasks.every((t) => t.state === "draft")).toBe(true);
  });

  it("--template with unknown name errors before creating anything", async () => {
    const deps = services();
    await expect(
      missionNew(deps, {
        title: "ghost",
        slug: "ghost",
        mode: "template",
        template: "ghost-template",
      }),
    ).rejects.toBeInstanceOf(MissionTemplateUnknownError);
    const missions = await deps.missionStore.list();
    expect(missions).toEqual([]);
  });

  it("--from-file loads a JSON batch and lands at planned", async () => {
    const batchPath = join(tmpDir, "batch.json");
    await writeFile(
      batchPath,
      JSON.stringify([
        { title: "First", slug: "first" },
        { title: "Second", slug: "second" },
      ]),
      "utf8",
    );
    const result = await missionNew(services(), {
      title: "From file",
      slug: "from-file",
      mode: "from-file",
      fromFile: batchPath,
    });
    expect(result.mission.state).toBe("planned");
    expect(result.tasks.length).toBe(2);
  });

  it("--from-file rejects an empty array", async () => {
    const batchPath = join(tmpDir, "batch.json");
    await writeFile(batchPath, JSON.stringify([]), "utf8");
    await expect(
      missionNew(services(), {
        title: "Empty",
        slug: "empty",
        mode: "from-file",
        fromFile: batchPath,
      }),
    ).rejects.toThrow();
  });

  it("--from-spec lands the mission at approved", async () => {
    const specPath = join(tmpDir, ".maestro/specs/demo-plan.md");
    await mkdir(join(tmpDir, ".maestro/specs"), { recursive: true });
    await writeFile(specPath, HEAVY_SPEC, "utf8");
    const result = await missionNew(services(), {
      title: "ignored — spec slug wins",
      slug: "ignored",
      mode: "from-spec",
      fromSpec: ".maestro/specs/demo-plan.md",
    });
    expect(result.mission.state).toBe("approved");
    expect(result.mission.slug).toBe("demo-plan");
    expect(result.tasks).toEqual([]);
  });

  it("rejects a duplicate slug with a clear error and leaves no orphan tasks", async () => {
    const deps = services();
    await missionNew(deps, {
      title: "First",
      slug: "dup",
      mode: "bare",
    });
    await expect(
      missionNew(deps, {
        title: "Second",
        slug: "dup",
        mode: "template",
        template: "refactor",
      }),
    ).rejects.toBeInstanceOf(DuplicateMissionSlugError);
    // The first mission persists; the second must not have created any tasks.
    const missions = await deps.missionStore.list();
    expect(missions.length).toBe(1);
    const tasks = await deps.taskStore.list();
    expect(tasks).toEqual([]);
  });

  it("--from-file rejects malformed JSON before creating the mission", async () => {
    const batchPath = join(tmpDir, "broken.json");
    await writeFile(batchPath, "{not-json", "utf8");
    const deps = services();
    await expect(
      missionNew(deps, {
        title: "Broken",
        slug: "broken",
        mode: "from-file",
        fromFile: batchPath,
      }),
    ).rejects.toThrow();
    const missions = await deps.missionStore.list();
    expect(missions).toEqual([]);
  });

  it("rejects an off-shape --slug before creating anything", async () => {
    // Slug shape gate: user-supplied --slug must pass SPEC_SLUG_PATTERN so an
    // exotic value (path traversal, whitespace, too short) can't propagate into
    // child task slugs or the on-disk mission row.
    const cases = [
      "x", // too short (min 3)
      "Has Caps", // uppercase + space
      "../etc", // path traversal
      "-leading-hyphen",
      "trailing-hyphen-",
      "double--hyphen",
    ];
    for (const bad of cases) {
      const deps = services();
      await expect(
        missionNew(deps, { title: "ok", slug: bad, mode: "bare" }),
      ).rejects.toBeInstanceOf(MissionNewInvalidFlagsError);
      const missions = await deps.missionStore.list();
      expect(missions).toEqual([]);
    }
  });

  it("user-defined template overrides built-in", async () => {
    const tplDir = join(tmpDir, ".maestro", "templates", "missions");
    await mkdir(tplDir, { recursive: true });
    await writeFile(
      join(tplDir, "refactor.yaml"),
      `name: refactor
description: Custom refactor variant
seedTasks:
  - title: Custom only
    slug: only
`,
      "utf8",
    );
    const result = await missionNew(services(), {
      title: "Refactor x",
      slug: "refactor-x",
      mode: "template",
      template: "refactor",
    });
    expect(result.tasks.length).toBe(1);
    expect(result.tasks[0]!.title).toBe("Custom only");
  });
});

describe("slugifyTitle", () => {
  it("kebab-cases a normal title", () => {
    expect(slugifyTitle("Refactor Auth Module")).toBe("refactor-auth-module");
  });

  it("strips trailing punctuation", () => {
    expect(slugifyTitle("Hello, world!")).toBe("hello-world");
  });

  it("throws when title slugs to empty (no fallback to time-based slug)", () => {
    expect(() => slugifyTitle("!!!")).toThrow();
  });

  it("truncates very long titles", () => {
    const long = "a".repeat(120);
    expect(slugifyTitle(long).length).toBeLessThanOrEqual(64);
  });
});
