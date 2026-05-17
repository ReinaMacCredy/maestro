import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { listTemplates, loadTemplate } from "@/features/mission/templates/loader.js";
import { MissionTemplateLoadError } from "@/features/mission/domain/template-types.js";
import { BUILTIN_TEMPLATES } from "@/features/mission/templates/builtin.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-tpl-test-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function writeTemplate(name: string, body: string): Promise<string> {
  const dir = join(tmpDir, ".maestro", "templates", "missions");
  await mkdir(dir, { recursive: true });
  const path = join(dir, `${name}.yaml`);
  await writeFile(path, body, "utf8");
  return path;
}

describe("mission template loader (user dir)", () => {
  it("loads a well-formed user template", async () => {
    await writeTemplate(
      "spike",
      `name: spike
description: Quick exploration
seedTasks:
  - title: Investigate
    slug: investigate
  - title: Write findings
    slug: findings
`,
    );
    const tpl = await loadTemplate("spike", tmpDir);
    expect(tpl).toBeDefined();
    expect(tpl!.name).toBe("spike");
    expect(tpl!.source).toBe("user");
    expect(tpl!.seedTasks.length).toBe(2);
  });

  it("user file overrides built-in of the same name", async () => {
    await writeTemplate(
      "refactor",
      `name: refactor
description: My custom refactor
seedTasks:
  - title: Custom step
    slug: custom-step
`,
    );
    const tpl = await loadTemplate("refactor", tmpDir);
    expect(tpl!.description).toBe("My custom refactor");
    expect(tpl!.seedTasks.length).toBe(1);
    expect(tpl!.source).toBe("user");
  });

  it("falls back to built-in when user dir has no matching file", async () => {
    const tpl = await loadTemplate("refactor", tmpDir);
    expect(tpl).toBeDefined();
    expect(tpl!.source).toBe("builtin");
  });

  it("returns undefined for unknown names", async () => {
    const tpl = await loadTemplate("nonexistent-template", tmpDir);
    expect(tpl).toBeUndefined();
  });

  it("rejects when name field doesn't match filename stem", async () => {
    await writeTemplate(
      "mismatch",
      `name: actually-different
description: x
seedTasks:
  - title: T
    slug: tt
`,
    );
    await expect(loadTemplate("mismatch", tmpDir)).rejects.toBeInstanceOf(
      MissionTemplateLoadError,
    );
  });

  it("rejects empty seedTasks", async () => {
    await writeTemplate(
      "empty",
      `name: empty
description: x
seedTasks: []
`,
    );
    await expect(loadTemplate("empty", tmpDir)).rejects.toBeInstanceOf(MissionTemplateLoadError);
  });

  it("rejects duplicate slugs within a template", async () => {
    await writeTemplate(
      "dupes",
      `name: dupes
description: x
seedTasks:
  - title: A
    slug: same
  - title: B
    slug: same
`,
    );
    await expect(loadTemplate("dupes", tmpDir)).rejects.toBeInstanceOf(MissionTemplateLoadError);
  });

  it("rejects extra top-level fields (strict parse)", async () => {
    await writeTemplate(
      "extra",
      `name: extra
description: x
seedTasks:
  - title: A
    slug: a
unknown_field: should-not-pass-strict
`,
    );
    await expect(loadTemplate("extra", tmpDir)).rejects.toBeInstanceOf(MissionTemplateLoadError);
  });

  it("rejects extra fields inside seedTasks entries", async () => {
    await writeTemplate(
      "task-extra",
      `name: task-extra
description: x
seedTasks:
  - title: A
    slug: a
    secret: nope
`,
    );
    await expect(loadTemplate("task-extra", tmpDir)).rejects.toBeInstanceOf(
      MissionTemplateLoadError,
    );
  });

  it("rejects malformed YAML with a clear error", async () => {
    await writeTemplate("broken", "name: : : invalid\nseedTasks: [\n");
    await expect(loadTemplate("broken", tmpDir)).rejects.toBeInstanceOf(MissionTemplateLoadError);
  });

  it("rejects non-kebab-case slug", async () => {
    await writeTemplate(
      "bad-slug",
      `name: bad-slug
description: x
seedTasks:
  - title: A
    slug: NotKebab
`,
    );
    await expect(loadTemplate("bad-slug", tmpDir)).rejects.toBeInstanceOf(
      MissionTemplateLoadError,
    );
  });
});

describe("mission template listTemplates", () => {
  it("returns only built-ins when user dir is missing", async () => {
    const listed = await listTemplates(tmpDir);
    expect(listed.builtin.length).toBe(BUILTIN_TEMPLATES.length);
    expect(listed.user).toEqual([]);
    expect(listed.overrides).toEqual([]);
  });

  it("returns user templates and marks overrides", async () => {
    await writeTemplate(
      "spike",
      `name: spike
description: x
seedTasks:
  - title: T
    slug: tt
`,
    );
    await writeTemplate(
      "refactor",
      `name: refactor
description: custom
seedTasks:
  - title: T
    slug: tt
`,
    );
    const listed = await listTemplates(tmpDir);
    expect(listed.user.map((t) => t.name).sort()).toEqual(["refactor", "spike"]);
    expect(listed.overrides).toEqual(["refactor"]);
  });
});
