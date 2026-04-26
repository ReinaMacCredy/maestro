import { describe, expect, it } from "bun:test";
import { readdir, stat } from "node:fs/promises";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { BUNDLED_SKILL_TEMPLATES } from "@/infra/domain/bundled-skill-templates.js";
import { isIgnoredSkillSourceArtifact } from "../../../../scripts/skill-template-source-lib";

const ROOT = fileURLToPath(new URL("../../../..", import.meta.url));
const SOURCE_DIR = join(ROOT, "skills", "bundled");
const EXECUTABLE_EXTENSIONS = [".sh", ".bash", ".command", ".cmd", ".bat", ".ps1"] as const;

async function listFilesRecursive(dir: string): Promise<string[]> {
  const entries = (await readdir(dir, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));
  const files: string[] = [];
  for (const entry of entries) {
    const absolute = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFilesRecursive(absolute)));
      continue;
    }
    if (entry.isFile() && !isIgnoredSkillSourceArtifact(entry.name)) files.push(absolute);
  }
  return files;
}

function normalize(text: string): string {
  return text.replace(/\r\n/g, "\n");
}

function isExecutableFile(path: string, mode: number): boolean {
  if ((mode & 0o111) !== 0) return true;
  if (process.platform !== "win32") return false;
  return EXECUTABLE_EXTENSIONS.some((extension) => path.endsWith(extension));
}

describe("BUNDLED_SKILL_TEMPLATES", () => {
  it("matches the files under skills/bundled/ (no drift)", async () => {
    const skillDirs = (await readdir(SOURCE_DIR, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort((a, b) => a.localeCompare(b));

    expect(BUNDLED_SKILL_TEMPLATES.map((template) => template.name)).toEqual(skillDirs);

    for (const dirName of skillDirs) {
      const template = BUNDLED_SKILL_TEMPLATES.find((t) => t.name === dirName);
      expect(template, `template for ${dirName}`).toBeDefined();

      const skillDir = join(SOURCE_DIR, dirName);
      const absolutes = await listFilesRecursive(skillDir);
      const resolvedDisk = await Promise.all(
        absolutes.map(async (abs) => ({
          path: relative(skillDir, abs).split(sep).join("/"),
          content: normalize(await Bun.file(abs).text()),
          executable: isExecutableFile(
            relative(skillDir, abs).split(sep).join("/"),
            (await stat(abs)).mode,
          ),
        })),
      );

      expect(template!.files.length, `${dirName} file count`).toBe(resolvedDisk.length);
      for (const diskFile of resolvedDisk) {
        const match = template!.files.find((f) => f.path === diskFile.path);
        expect(match, `${dirName}/${diskFile.path} present in template`).toBeDefined();
        expect(match!.content, `${dirName}/${diskFile.path} content`).toBe(diskFile.content);
        expect(match!.executable === true, `${dirName}/${diskFile.path} executable`).toBe(diskFile.executable);
      }
    }
  });

  it("ships the expected bundled skills", () => {
    const names = BUNDLED_SKILL_TEMPLATES.map((t) => t.name).sort();
    expect(names).toEqual([
      "maestro-brainstorm",
      "maestro-handoff",
      "maestro-mission",
      "maestro-plan",
      "maestro-setup",
      "maestro-task",
    ]);
  });

  it("each skill has SKILL.md with matching frontmatter name", async () => {
    for (const template of BUNDLED_SKILL_TEMPLATES) {
      const skillMd = template.files.find((f) => f.path === "SKILL.md");
      expect(skillMd, `${template.name}/SKILL.md`).toBeDefined();
      expect(skillMd!.content).toContain(`name: ${template.name}`);
    }
  });

  it("no absolute /Users/ paths in bundled skills", () => {
    for (const template of BUNDLED_SKILL_TEMPLATES) {
      for (const file of template.files) {
        if (!file.path.endsWith(".md") && !file.path.endsWith(".yaml")) continue;
        expect(file.content, `${template.name}/${file.path}`).not.toContain("/Users/");
      }
    }
  });

  it("chain references are consistent", () => {
    const brainstorm = BUNDLED_SKILL_TEMPLATES.find((t) => t.name === "maestro-brainstorm");
    const plan = BUNDLED_SKILL_TEMPLATES.find((t) => t.name === "maestro-plan");

    const brainstormSkill = brainstorm!.files.find((f) => f.path === "SKILL.md")!;
    expect(brainstormSkill.content).not.toContain("preplan-brainstorm");
    expect(brainstormSkill.content).not.toContain("execution-plan");
    expect(brainstormSkill.content).toContain("maestro-plan");

    const planSkill = plan!.files.find((f) => f.path === "SKILL.md")!;
    expect(planSkill.content).not.toContain("preplan-brainstorm");
    expect(planSkill.content).toContain("maestro-brainstorm");
    expect(planSkill.content).toContain("maestro-task");
    expect(planSkill.content).toContain("maestro-handoff");
    expect(planSkill.content).toContain("## Persist the plan");
  });

  it("ships maestro-setup with managed-marker and report contracts", () => {
    const setup = BUNDLED_SKILL_TEMPLATES.find((template) => template.name === "maestro-setup");
    expect(setup).toBeDefined();

    const skill = setup!.files.find((file) => file.path === "SKILL.md")!;
    expect(skill.content).toContain("name: maestro-setup");
    expect(skill.content).toContain("Skill-first, CLI-second");
    expect(skill.content).toContain("<!-- maestro-setup:start -->");
    expect(skill.content).toContain("<!-- maestro-setup:end -->");
    expect(skill.content).toContain("<!-- maestro-setup:generated:start -->");
    expect(skill.content).toContain("<!-- maestro-setup:generated:end -->");
    expect(skill.content).toContain(`<!-- maestro-setup:start -->
## Maestro Context

Before non-trivial work:
- Load \`.maestro/context/index.md\` first.
- Open only the specific context docs relevant to the task.
- Follow detected language guides under \`.maestro/context/code_styleguides/\`.
- Preserve user content outside managed setup sections.
- If context docs conflict with closer repo instructions, follow the closer
  instruction file and report the conflict.
<!-- maestro-setup:end -->`);
    expect(skill.content).toContain(".maestro/setup-report.md");
    expect(skill.content).toContain("maestro setup --dry-run --json");

    const planningTemplate = setup!.files.find((file) => file.path === "reference/context-templates/planning.md");
    expect(planningTemplate?.content).toContain("Approved implementation plans live under `.maestro/plans/`");
    expect(planningTemplate?.content).toContain("Convert plan phases into `maestro task` entries");

    const reportTemplate = setup!.files.find((file) => file.path === "reference/setup-report-template.md");
    expect(reportTemplate?.content).toContain("## Evidence Sources");
    expect(reportTemplate?.content).toContain("## TODOs Left");
    expect(reportTemplate?.content).toContain("## Warnings");
  });

  it("ships maestro-setup Google styleguide snapshots with attribution", () => {
    const setup = BUNDLED_SKILL_TEMPLATES.find((template) => template.name === "maestro-setup");
    expect(setup).toBeDefined();

    const expectedGuides = [
      "angularjs.md",
      "common-lisp.md",
      "cpp.md",
      "csharp.md",
      "go.md",
      "html-css.md",
      "javascript.md",
      "java.md",
      "json.md",
      "markdown.md",
      "objective-c.md",
      "python.md",
      "r.md",
      "shell.md",
      "swift.md",
      "typescript.md",
      "vimscript.md",
      "xml.md",
    ];

    for (const guide of expectedGuides) {
      const file = setup!.files.find((entry) => entry.path === `reference/styleguides/${guide}`);
      expect(file, `reference/styleguides/${guide}`).toBeDefined();
      expect(file!.content).toContain("Snapshot date: 2026-04-24");
      expect(file!.content).toContain("Creative Commons Attribution 3.0");
      expect(file!.content).toContain("google.github.io");
    }

    const jsonGuide = setup!.files.find((entry) => entry.path === "reference/styleguides/json.md");
    expect(jsonGuide?.content).toContain("code samples are Apache 2.0");

    const index = setup!.files.find((entry) => entry.path === "reference/styleguides/INDEX.md");
    expect(index?.content).toContain("excludes external Dart and Kotlin guides");
    expect(index?.content).toContain("code samples under Apache 2.0");
  });

  it("marks bundled shell helpers as executable", () => {
    const brainstorm = BUNDLED_SKILL_TEMPLATES.find((template) => template.name === "maestro-brainstorm");
    expect(brainstorm?.files.find((file) => file.path === "scripts/start-server.sh")?.executable).toBe(true);
    expect(brainstorm?.files.find((file) => file.path === "scripts/stop-server.sh")?.executable).toBe(true);
  });
});
