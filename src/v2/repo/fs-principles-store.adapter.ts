import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  isValidPrincipleSlug,
  PrincipleParseError,
  type Principle,
} from "../types/principle.js";
import type { PrinciplesStorePort } from "./principles-store.port.js";

const DEFAULT_PRINCIPLES_DIR = "docs/principles";
const LEGACY_SUBDIR = "legacy";

export interface FsPrinciplesStoreOptions {
  readonly repoRoot: string;
  readonly subdir?: string;
}

const SECTION_HEADERS = ["Rule", "Rationale", "Scan Command", "Fix Recipe"] as const;
type SectionHeader = (typeof SECTION_HEADERS)[number];

export function parsePrincipleFile(raw: string, slug: string, path: string): Principle {
  const sections = splitSections(raw);
  for (const header of SECTION_HEADERS) {
    if (sections[header] === undefined) {
      throw new PrincipleParseError(
        `Principle ${path} missing required section: "## ${header}"`,
        header,
      );
    }
  }
  return {
    slug,
    rule: sections.Rule!.trim(),
    rationale: sections.Rationale!.trim(),
    scan_command: sections["Scan Command"]!.trim(),
    fix_recipe: sections["Fix Recipe"]!.trim(),
  };
}

function splitSections(raw: string): Partial<Record<SectionHeader, string>> {
  const lines = raw.split("\n");
  const out: Partial<Record<SectionHeader, string>> = {};
  let currentHeader: SectionHeader | undefined;
  let buffer: string[] = [];

  const flush = (): void => {
    if (currentHeader !== undefined) {
      out[currentHeader] = buffer.join("\n");
    }
    buffer = [];
  };

  for (const line of lines) {
    const match = /^##\s+(.+?)\s*$/.exec(line);
    if (match) {
      const header = match[1] as SectionHeader;
      if ((SECTION_HEADERS as readonly string[]).includes(header)) {
        flush();
        currentHeader = header;
        continue;
      }
    }
    if (currentHeader !== undefined) {
      buffer.push(line);
    }
  }
  flush();
  return out;
}

export class FsPrinciplesStore implements PrinciplesStorePort {
  readonly #dir: string;

  constructor(options: FsPrinciplesStoreOptions) {
    this.#dir = join(options.repoRoot, options.subdir ?? DEFAULT_PRINCIPLES_DIR);
  }

  pathFor(slug: string): string {
    return join(this.#dir, `${slug}.md`);
  }

  async list(): Promise<readonly Principle[]> {
    let entries: string[];
    try {
      entries = await readdir(this.#dir);
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return [];
      throw err;
    }
    const slugs = entries
      .filter((name) => name.endsWith(".md") && name !== `${LEGACY_SUBDIR}.md`)
      .map((name) => name.slice(0, -3))
      .filter(isValidPrincipleSlug);
    const out: Principle[] = [];
    for (const slug of slugs) {
      const principle = await this.get(slug);
      if (principle !== undefined) out.push(principle);
    }
    return out;
  }

  async get(slug: string): Promise<Principle | undefined> {
    const path = this.pathFor(slug);
    let raw: string;
    try {
      raw = await readFile(path, "utf8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") return undefined;
      throw err;
    }
    return parsePrincipleFile(raw, slug, path);
  }

  async write(slug: string, content: string): Promise<void> {
    if (!isValidPrincipleSlug(slug)) {
      throw new PrincipleParseError(
        `Principle slug must be kebab-case (2..64 chars): ${slug}`,
        "slug",
      );
    }
    await mkdir(this.#dir, { recursive: true });
    await writeFile(this.pathFor(slug), content, "utf8");
  }
}
