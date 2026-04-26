import { beforeEach, describe, expect, it } from "bun:test";
import { chmod, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  collectSkillTemplates,
  isIgnoredSkillSourceArtifact,
  normalizeLineEndings,
} from "../../../scripts/skill-template-source-lib";

describe("skill template source collection", () => {
  let tmpDir: string;
  let sourceDir: string;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "skill-source-"));
    sourceDir = join(tmpDir, "skills");
    await mkdir(join(sourceDir, "demo-skill", "scripts"), { recursive: true });
  });

  it("normalizes CRLF content to LF for cross-platform generated templates", () => {
    expect(normalizeLineEndings("line one\r\nline two\r\n")).toBe("line one\nline two\n");
  });

  it("preserves LF content as-is", () => {
    expect(normalizeLineEndings("line one\nline two\n")).toBe("line one\nline two\n");
  });

  it("ignores local OS and editor artifacts without dropping real files", async () => {
    await writeFile(join(sourceDir, "demo-skill", "SKILL.md"), "# Demo\n", "utf8");
    await writeFile(join(sourceDir, "demo-skill", ".DS_Store"), "noise", "utf8");
    await writeFile(join(sourceDir, "demo-skill", "draft.md.swp"), "noise", "utf8");
    await writeFile(join(sourceDir, "demo-skill", "notes.md~"), "noise", "utf8");

    const templates = await collectSkillTemplates({
      sourceDir,
      rootDir: tmpDir,
      errorScope: "skills/",
    });

    expect(templates).toHaveLength(1);
    expect(templates[0]?.files.map((file) => file.path)).toEqual(["SKILL.md"]);
  });

  it("fails loudly on non-UTF-8 source files that are not ignored", async () => {
    await writeFile(join(sourceDir, "demo-skill", "SKILL.md"), new Uint8Array([0xff, 0xfe, 0xfd]));

    await expect(collectSkillTemplates({
      sourceDir,
      rootDir: tmpDir,
      errorScope: "skills/",
    })).rejects.toThrow("Non-UTF-8 content under skills/");
  });

  it("preserves executable metadata when requested", async () => {
    const helper = join(sourceDir, "demo-skill", "scripts", "run.sh");
    await writeFile(join(sourceDir, "demo-skill", "SKILL.md"), "# Demo\n", "utf8");
    await writeFile(helper, "#!/usr/bin/env bash\n", "utf8");
    await chmod(helper, 0o755);

    const templates = await collectSkillTemplates({
      sourceDir,
      rootDir: tmpDir,
      errorScope: "skills/",
      includeExecutableMetadata: true,
    });

    expect(templates[0]?.files.find((file) => file.path === "scripts/run.sh")?.executable).toBe(true);
    expect(templates[0]?.files.find((file) => file.path === "SKILL.md")?.executable).toBeUndefined();
  });

  it("maps directory names when a skill family encodes names on disk", async () => {
    await writeFile(join(sourceDir, "demo-skill", "SKILL.md"), "# Demo\n", "utf8");

    const templates = await collectSkillTemplates({
      sourceDir,
      rootDir: tmpDir,
      errorScope: "skills/",
      mapSkillName: (dirName) => `mapped:${dirName}`,
    });

    expect(templates[0]?.name).toBe("mapped:demo-skill");
  });
});

describe("isIgnoredSkillSourceArtifact", () => {
  it("recognizes local artifact filenames", () => {
    expect(isIgnoredSkillSourceArtifact(".DS_Store")).toBe(true);
    expect(isIgnoredSkillSourceArtifact("Thumbs.db")).toBe(true);
    expect(isIgnoredSkillSourceArtifact("notes.md.swp")).toBe(true);
    expect(isIgnoredSkillSourceArtifact("notes.md")).toBe(false);
  });
});
