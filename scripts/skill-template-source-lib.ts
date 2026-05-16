import { readdir, readFile, stat } from "node:fs/promises";
import { basename, join, relative, sep } from "node:path";

const UTF8_STRICT = new TextDecoder("utf-8", { fatal: true });
const EXECUTABLE_EXTENSIONS = [".sh", ".bash", ".command", ".cmd", ".bat", ".ps1"] as const;
const IGNORED_ARTIFACT_NAMES = new Set([".DS_Store", "Thumbs.db", "ehthumbs.db"]);
const IGNORED_ARTIFACT_SUFFIXES = [".swp", ".swo", "~"] as const;

export interface SkillTemplateFile {
  readonly path: string;
  readonly content: string;
  readonly executable?: boolean;
}

export interface SkillTemplate {
  readonly name: string;
  readonly files: readonly SkillTemplateFile[];
}

export interface CollectSkillTemplatesOptions {
  readonly sourceDir: string;
  readonly rootDir: string;
  readonly errorScope: string;
  readonly mapSkillName?: (dirName: string) => string;
  readonly includeExecutableMetadata?: boolean;
}

export function normalizeLineEndings(text: string | undefined): string | undefined {
  return text?.replace(/\r\n/g, "\n");
}

export function isIgnoredSkillSourceArtifact(path: string): boolean {
  const name = basename(path);
  return IGNORED_ARTIFACT_NAMES.has(name)
    || IGNORED_ARTIFACT_SUFFIXES.some((suffix) => name.endsWith(suffix));
}

export async function collectSkillTemplates(
  options: CollectSkillTemplatesOptions,
): Promise<readonly SkillTemplate[]> {
  let entries;
  try {
    entries = await readdir(options.sourceDir, { withFileTypes: true });
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw err;
  }
  const skillDirs = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right, "en"));

  const templates: SkillTemplate[] = [];
  for (const dirName of skillDirs) {
    const skillDir = join(options.sourceDir, dirName);
    const absolutePaths = await listIncludedFilesRecursive(skillDir);
    const files: SkillTemplateFile[] = [];
    for (const absolute of absolutePaths) {
      const relativePath = relative(skillDir, absolute).split(sep).join("/");
      const content = normalizeLineEndings(await readStrictUtf8(absolute, options)) ?? "";
      const executable = options.includeExecutableMetadata === true
        ? await isExecutableFile(absolute, relativePath)
        : false;
      files.push({
        path: relativePath,
        content,
        ...(executable ? { executable: true } : {}),
      });
    }
    templates.push({
      name: options.mapSkillName?.(dirName) ?? dirName,
      files,
    });
  }
  return templates;
}

async function listIncludedFilesRecursive(dir: string): Promise<string[]> {
  const entries = (await readdir(dir, { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name, "en"));
  const files: string[] = [];
  for (const entry of entries) {
    const absolute = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...await listIncludedFilesRecursive(absolute));
      continue;
    }
    if (entry.isFile() && !isIgnoredSkillSourceArtifact(entry.name)) {
      files.push(absolute);
    }
  }
  return files;
}

async function readStrictUtf8(path: string, options: CollectSkillTemplatesOptions): Promise<string> {
  const bytes = await readFile(path);
  try {
    return UTF8_STRICT.decode(bytes);
  } catch {
    throw new Error(`Non-UTF-8 content under ${options.errorScope}: ${relative(options.rootDir, path)}`);
  }
}

async function isExecutableFile(path: string, relativePath: string): Promise<boolean> {
  const mode = (await stat(path)).mode;
  if ((mode & 0o111) !== 0) return true;
  if (process.platform !== "win32") return false;
  return EXECUTABLE_EXTENSIONS.some((extension) => relativePath.endsWith(extension));
}
