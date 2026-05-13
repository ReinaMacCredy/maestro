import { join, dirname, isAbsolute, relative } from "node:path";
import { Glob } from "bun";
import { fileExists, readText } from "@/shared/lib/fs.js";
import { recordEvidence, type EvidenceStorePort } from "@/features/evidence";

export type StaleReferenceKind =
  | "missing-file"
  | "missing-symbol"
  | "moved-path"
  | "broken-link";

export interface StaleReference {
  readonly file: string;
  readonly line: number;
  readonly reference: string;
  readonly kind: StaleReferenceKind;
}

export interface DocGardeningDeps {
  readonly evidenceStore?: EvidenceStorePort;
  readonly fileExists?: (path: string) => Promise<boolean>;
  readonly readText?: (path: string) => Promise<string | undefined>;
  readonly listFiles?: (root: string, patterns: readonly string[]) => Promise<readonly string[]>;
}

export interface DocGardeningArgs {
  readonly projectRoot: string;
  readonly taskId?: string;
  readonly recordEvidence?: boolean;
}

export interface DocGardeningResult {
  readonly scannedFiles: number;
  readonly staleReferences: readonly StaleReference[];
  readonly evidenceId?: string;
}

const DEFAULT_PATTERNS = [
  "AGENTS.md",
  "CLAUDE.md",
  "README.md",
  "docs/**/*.md",
  ".maestro/**/*.md",
  "skills/**/*.md",
];

const PATH_REFERENCE_RE =
  /(?:`|^|\s|\()((?:\.\.?\/|\/)?(?:src|scripts|docs|skills|\.maestro|\.factory|tests|hooks|apps)\/[A-Za-z0-9_./-]+\.[a-z0-9]+)(?:`|\)|\s|,|;|:|$)/gm;

const MARKDOWN_LINK_RE = /\[[^\]]+\]\(([^)]+)\)/g;

interface CandidateRef {
  readonly file: string;
  readonly line: number;
  readonly reference: string;
  readonly target: string;
}

export async function scanDocGardening(
  deps: DocGardeningDeps,
  args: DocGardeningArgs,
): Promise<DocGardeningResult> {
  const list = deps.listFiles ?? defaultListFiles;
  const exists = deps.fileExists ?? fileExists;
  const read = deps.readText ?? readText;

  const files = await list(args.projectRoot, DEFAULT_PATTERNS);

  const candidates: CandidateRef[] = [];
  await Promise.all(
    files.map(async (file): Promise<void> => {
      const absFile = isAbsolute(file) ? file : join(args.projectRoot, file);
      const text = (await read(absFile)) ?? "";
      const lines = text.split("\n");
      for (let i = 0; i < lines.length; i++) {
        const lineText = lines[i] ?? "";
        for (const ref of collectReferences(lineText)) {
          if (isExternal(ref) || isAnchor(ref)) continue;
          const target = resolveReference(ref, args.projectRoot, absFile);
          if (target === undefined) continue;
          candidates.push({
            file: relative(args.projectRoot, absFile),
            line: i + 1,
            reference: ref,
            target,
          });
        }
      }
    }),
  );

  const uniqueTargets = [...new Set(candidates.map((c) => c.target))];
  const existence = new Map<string, boolean>();
  await Promise.all(
    uniqueTargets.map(async (t): Promise<void> => {
      existence.set(t, await exists(t));
    }),
  );

  const stale: StaleReference[] = [];
  for (const c of candidates) {
    if (existence.get(c.target) !== true) {
      stale.push({
        file: c.file,
        line: c.line,
        reference: c.reference,
        kind: classifyKind(c.reference),
      });
    }
  }

  let evidenceId: string | undefined;
  if (deps.evidenceStore !== undefined && args.taskId !== undefined && args.recordEvidence !== false) {
    const row = await recordEvidence(deps.evidenceStore, {
      task_id: args.taskId,
      kind: "doc-gardening",
      witness_level: "witnessed-by-maestro",
      payload: { staleReferences: stale, scannedFiles: files.length },
    });
    evidenceId = row.id;
  }

  return { scannedFiles: files.length, staleReferences: stale, evidenceId };
}

function collectReferences(line: string): readonly string[] {
  const out: string[] = [];
  for (const m of line.matchAll(PATH_REFERENCE_RE)) {
    if (m[1]) out.push(m[1]);
  }
  for (const m of line.matchAll(MARKDOWN_LINK_RE)) {
    if (m[1]) out.push(m[1]);
  }
  return out;
}

function isExternal(ref: string): boolean {
  return /^(https?:|mailto:|ftp:|git@|github\.com)/i.test(ref);
}

function isAnchor(ref: string): boolean {
  return ref.startsWith("#");
}

function classifyKind(ref: string): StaleReferenceKind {
  if (ref.endsWith(".md")) return "broken-link";
  return "missing-file";
}

function resolveReference(ref: string, root: string, fromFile: string): string | undefined {
  const cleaned = ref.split("#")[0]?.split("?")[0] ?? "";
  if (cleaned.length === 0) return undefined;
  if (cleaned.startsWith("/")) return join(root, cleaned.slice(1));
  if (cleaned.startsWith("./") || cleaned.startsWith("../")) {
    return join(dirname(fromFile), cleaned);
  }
  return join(root, cleaned);
}

async function defaultListFiles(root: string, patterns: readonly string[]): Promise<readonly string[]> {
  const out = new Set<string>();
  for (const pattern of patterns) {
    const glob = new Glob(pattern);
    for await (const file of glob.scan({ cwd: root, dot: true, onlyFiles: true })) {
      out.add(join(root, file));
    }
  }
  return [...out];
}
