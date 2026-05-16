import { isValidPrincipleSlug, PrincipleParseError } from "../types/principle.js";
import type {
  EvidenceStorePort,
  LintViolationEvidenceRow,
} from "../repo/evidence-store.port.js";
import type { PrinciplesStorePort } from "../repo/principles-store.port.js";

export interface PrinciplePromoteDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly principlesStore: PrinciplesStorePort;
}

export interface PrinciplePromoteInput {
  readonly correction_id: string;
}

export interface PrinciplePromoteResult {
  readonly slug: string;
  readonly path: string;
  readonly correction_id: string;
  readonly rule_id: string;
  readonly content: string;
}

export class CorrectionNotFoundError extends Error {
  readonly correction_id: string;
  constructor(correction_id: string) {
    super(`Correction ${correction_id} not found in evidence store`);
    this.name = "CorrectionNotFoundError";
    this.correction_id = correction_id;
  }
}

export class CorrectionNotLintViolationError extends Error {
  readonly correction_id: string;
  readonly kind: string;
  constructor(correction_id: string, kind: string) {
    super(
      `Correction ${correction_id} has kind ${JSON.stringify(kind)}; only lint-violation rows can be promoted to principles`,
    );
    this.name = "CorrectionNotLintViolationError";
    this.correction_id = correction_id;
    this.kind = kind;
  }
}

export async function principlePromote(
  deps: PrinciplePromoteDeps,
  input: PrinciplePromoteInput,
): Promise<PrinciplePromoteResult> {
  const rows = await deps.evidenceStore.list();
  const row = rows.find((r) => r.id === input.correction_id);
  if (!row) throw new CorrectionNotFoundError(input.correction_id);
  if (row.kind !== "lint-violation") {
    throw new CorrectionNotLintViolationError(input.correction_id, row.kind);
  }

  const baseSlug = ruleIdToSlug(row.rule_id);
  if (!isValidPrincipleSlug(baseSlug)) {
    throw new PrincipleParseError(
      `rule_id ${JSON.stringify(row.rule_id)} maps to invalid principle slug ${JSON.stringify(baseSlug)}`,
      "slug",
    );
  }

  const slug = await resolveCollisionFreeSlug(deps.principlesStore, baseSlug);
  const content = renderPrincipleScaffold(slug, row);
  await deps.principlesStore.write(slug, content);
  return {
    slug,
    path: `docs/principles/${slug}.md`,
    correction_id: input.correction_id,
    rule_id: row.rule_id,
    content,
  };
}

export function ruleIdToSlug(ruleId: string): string {
  return ruleId.trim().toLowerCase().replaceAll("_", "-");
}

async function resolveCollisionFreeSlug(
  store: PrinciplesStorePort,
  baseSlug: string,
): Promise<string> {
  if (!(await store.exists(baseSlug))) return baseSlug;
  for (let n = 2; n < 1000; n++) {
    const candidate = `${baseSlug}-${n}`;
    if (!(await store.exists(candidate))) return candidate;
  }
  throw new Error(`unable to find unused collision suffix for principle slug ${baseSlug}`);
}

export function renderPrincipleScaffold(
  slug: string,
  row: LintViolationEvidenceRow,
): string {
  const locator = row.line !== undefined ? `${row.file}:${row.line}` : row.file;
  const remediation = (row.remediation ?? "").trim();
  const fixSection =
    remediation.length > 0
      ? remediation
      : "Investigate the original correction and document a concrete fix.";
  return [
    `# ${slug}`,
    "",
    "## Rule",
    "",
    row.message.trim().length > 0 ? row.message.trim() : "(fill in the rule)",
    "",
    "## Rationale",
    "",
    `Promoted from correction ${row.id} recorded at ${row.timestamp} against ${locator}.`,
    "",
    "## Scan Command",
    "",
    "# TODO: replace with a scan command that detects this class of violation.",
    "",
    "## Fix Recipe",
    "",
    fixSection,
    "",
  ].join("\n");
}
