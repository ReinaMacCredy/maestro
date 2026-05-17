import { mkdir, readFile, readdir, writeFile, access } from "node:fs/promises";
import { join } from "node:path";
import YAML from "yaml";
import {
  isRiskClass,
  isSpecMode,
  isWorkType,
  type ProductSpec,
  type ProductSpecFrontmatter,
} from "../types/product-spec.js";
import { isValidSpecSlug } from "../types/spec-id.js";
import {
  SpecAlreadyExistsError,
  SpecNotFoundError,
  SpecParseError,
  type SpecStorePort,
} from "./spec-store.port.js";

const FRONTMATTER_DELIM = "---";
const DEFAULT_SPECS_DIR = ".maestro/specs";

export interface FsSpecStoreOptions {
  readonly repoRoot: string;
  readonly subdir?: string;
}

export function parseSpecFile(raw: string, path: string): ProductSpec {
  const lines = raw.split("\n");
  if (lines[0]?.trim() !== FRONTMATTER_DELIM) {
    throw new SpecParseError(`Spec ${path} missing leading --- frontmatter delimiter`);
  }
  let endIdx = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i]?.trim() === FRONTMATTER_DELIM) {
      endIdx = i;
      break;
    }
  }
  if (endIdx === -1) {
    throw new SpecParseError(`Spec ${path} missing closing --- frontmatter delimiter`);
  }
  const yamlText = lines.slice(1, endIdx).join("\n");
  const body = lines.slice(endIdx + 1).join("\n").replace(/^\n+/, "");

  let parsed: unknown;
  try {
    parsed = YAML.parse(yamlText);
  } catch (err) {
    throw new SpecParseError(
      `Spec ${path} has invalid YAML frontmatter: ${(err as Error).message}`,
    );
  }

  return {
    frontmatter: validateFrontmatter(parsed, path),
    body,
    path,
  };
}

function validateFrontmatter(value: unknown, path: string): ProductSpecFrontmatter {
  if (value === null || typeof value !== "object") {
    throw new SpecParseError(`Spec ${path} frontmatter is not an object`);
  }
  const obj = value as Record<string, unknown>;

  const slug = obj.slug;
  if (!isValidSpecSlug(slug)) {
    throw new SpecParseError(
      `Spec ${path} has invalid slug (must be kebab-case, 3..64 chars): ${String(slug)}`,
      "slug",
    );
  }

  const acceptance = obj.acceptance_criteria;
  if (!isStringArray(acceptance) || acceptance.length === 0) {
    const hint = Array.isArray(acceptance) && acceptance.some((v) => v !== null && typeof v === "object")
      ? `Spec ${path} acceptance_criteria contains a non-string item — likely a YAML inline mapping. Quote any line containing { or [, e.g. - "GET /healthz returns { ok: true }"`
      : `Spec ${path} requires acceptance_criteria: non-empty string array`;
    throw new SpecParseError(hint, "acceptance_criteria");
  }

  const nonGoals = obj.non_goals;
  if (nonGoals !== undefined && !isStringArray(nonGoals)) {
    throw new SpecParseError(
      `Spec ${path} non_goals must be a string array when present`,
      "non_goals",
    );
  }

  if (!isRiskClass(obj.risk_class)) {
    throw new SpecParseError(
      `Spec ${path} requires risk_class: low | medium | high | critical`,
      "risk_class",
    );
  }

  if (!isSpecMode(obj.mode)) {
    throw new SpecParseError(`Spec ${path} requires mode: light | heavy`, "mode");
  }

  if (!isWorkType(obj.work_type)) {
    const guess = typeof obj.work_type === "string" ? workTypeSuggestion(obj.work_type) : undefined;
    const base = `Spec ${path} requires work_type: new-spec | spec-slice | change-request | initiative | maintenance | harness-improvement`;
    throw new SpecParseError(
      guess ? `${base}. Got "${String(obj.work_type)}" — did you mean "${guess}"?` : base,
      "work_type",
    );
  }

  return {
    slug,
    acceptance_criteria: acceptance,
    non_goals: nonGoals ?? [],
    risk_class: obj.risk_class,
    mode: obj.mode,
    work_type: obj.work_type,
  };
}

function workTypeSuggestion(input: string): string | undefined {
  const v = input.toLowerCase();
  if (v === "feature" || v === "feat" || v === "new") return "new-spec";
  if (v === "bug" || v === "fix" || v === "bugfix" || v === "patch") return "change-request";
  if (v === "chore" || v === "cleanup" || v === "refactor") return "maintenance";
  if (v === "epic" || v === "project") return "initiative";
  return undefined;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((v) => typeof v === "string");
}

export function serializeSpec(spec: ProductSpec): string {
  const yamlText = YAML.stringify(spec.frontmatter).trimEnd();
  const bodyText = spec.body.length === 0 ? "" : `\n${spec.body}${spec.body.endsWith("\n") ? "" : "\n"}`;
  return `---\n${yamlText}\n---\n${bodyText}`;
}

export class FsSpecStore implements SpecStorePort {
  readonly #dir: string;

  constructor(options: FsSpecStoreOptions) {
    this.#dir = join(options.repoRoot, options.subdir ?? DEFAULT_SPECS_DIR);
  }

  pathFor(slug: string): string {
    return join(this.#dir, `${slug}.md`);
  }

  async read(slug: string): Promise<ProductSpec> {
    const path = this.pathFor(slug);
    let raw: string;
    try {
      raw = await readFile(path, "utf8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") {
        throw new SpecNotFoundError(slug);
      }
      throw err;
    }
    return parseSpecFile(raw, path);
  }

  async write(spec: ProductSpec): Promise<void> {
    await mkdir(this.#dir, { recursive: true });
    await writeFile(this.pathFor(spec.frontmatter.slug), serializeSpec(spec), "utf8");
  }

  async exists(slug: string): Promise<boolean> {
    try {
      await access(this.pathFor(slug));
      return true;
    } catch {
      return false;
    }
  }

  async list(): Promise<readonly string[]> {
    let entries: string[];
    try {
      entries = await readdir(this.#dir);
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw err;
    }
    return entries
      .filter((name) => name.endsWith(".md"))
      .map((name) => name.slice(0, -3))
      .filter(isValidSpecSlug);
  }

  async create(spec: ProductSpec): Promise<void> {
    if (await this.exists(spec.frontmatter.slug)) {
      throw new SpecAlreadyExistsError(spec.frontmatter.slug);
    }
    await this.write(spec);
  }
}
