import { expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const lanesPath = join(root, "site", "src", "content", "docs", "concepts", "lanes.md");
const recipePath = join(root, "src", "plugins", "recipes", "slp.md");

interface Registry {
  headers: string[];
  rows: RegistryRow[];
}

interface RegistryRow {
  cells: string[];
  values: Record<string, string>;
}

interface NumberedTest {
  file: string;
  source: string;
}

const mechanismAllowlist: Record<string, string> = {
  COUNCIL_REQUEST: "Peer return status, not a runtime enforcement mechanism.",
  DECISION_REVIEW_DUE: "Attention finding label; it observes a due review rather than enforcing a boundary.",
  DECISION_STALE: "Attention finding label; it observes a stale draft rather than enforcing a boundary.",
  DEPENDENCY_REQUEST: "Peer return status, not a runtime enforcement mechanism.",
  DISPATCH_UNACCEPTED: "Attention finding label; it reports lifecycle state rather than refusing an action.",
  DISPATCH_UNRETURNED: "Attention finding label; it reports lifecycle state rather than refusing an action.",
  HANDBACK_UNREVIEWED: "Attention finding label; it reports lifecycle state rather than refusing an action.",
  HUMAN_DECISION_REQUIRED: "Attention finding label; it routes a decision rather than enforcing a boundary.",
  LEAD_COLLISION: "Attention finding label; it reports a collision rather than refusing an action.",
  LESSONS_PENDING: "Attention finding label; it observes pending corrections rather than refusing an action.",
  MAESTRO_GOLDEN_UPDATE:
    "Re-record switch for the scenario replay harness; it rewrites goldens rather than enforcing a boundary.",
  REOPEN_REQUEST: "Peer return status, not a runtime enforcement mechanism.",
  REPEATED_FAILURE: "Attention finding label; it aggregates failures rather than refusing an action.",
  SCOPE_COLLISION: "Attention finding label; it reports overlapping scopes rather than refusing an action.",
  STALLED_LEASE: "Attention finding label; it reports a stalled lease rather than refusing an action.",
};

function cells(line: string): string[] {
  return line.trim().slice(1, -1).split("|").map((cell) => cell.trim());
}

async function registry(): Promise<Registry> {
  const lines = (await readFile(lanesPath, "utf8")).split("\n");
  const headerIndex = lines.findIndex(
    (line) => line.startsWith("|") && line.includes("Boundary") && line.includes("Enforced by"),
  );
  if (headerIndex === -1) return { headers: [], rows: [] };
  const headers = cells(lines[headerIndex] as string);
  const rows: RegistryRow[] = [];
  for (const line of lines.slice(headerIndex + 2)) {
    if (!line.startsWith("|")) break;
    const rowCells = cells(line);
    rows.push({
      cells: rowCells,
      values: Object.fromEntries(headers.map((header, index) => [header, rowCells[index] ?? ""])),
    });
  }
  return { headers, rows };
}

async function numberedTests(): Promise<Map<string, NumberedTest[]>> {
  const tests = new Map<string, NumberedTest[]>();
  const names = (await readdir(import.meta.dir)).filter((name) => name.endsWith(".test.ts"));
  for (const name of names) {
    const source = await readFile(join(import.meta.dir, name), "utf8");
    const declarations = [...source.matchAll(/\btest\(\s*["'`](\d+)\b/g)];
    for (const [index, declaration] of declarations.entries()) {
      const number = declaration[1] as string;
      const start = declaration.index as number;
      const end = declarations[index + 1]?.index ?? source.length;
      const matches = tests.get(number) ?? [];
      matches.push({ file: name, source: source.slice(start, end) });
      tests.set(number, matches);
    }
  }
  return tests;
}

function citation(row: RegistryRow): string | null {
  return row.values.Proof?.match(/\btest (\d+)\b/)?.[1] ?? null;
}

function enforcingRows(rows: RegistryRow[]): RegistryRow[] {
  return rows.filter((row) => row.values["Enforced by"] !== "nothing");
}

const attackMarkers = [
  /new Database\s*\(/,
  /MAESTRO_SESSION_ID\s*:/,
  /session\(\s*["'`][^"'`]+["'`]\s*[,)]/,
  /\breadOnly\b/,
  /\b(?:bypass|perturb(?:ation)?|replacement|stranger)\b/i,
  /JSON\.stringify\(\{\s*tool_name:/,
  /open-open-return|open-return-open/,
];

const refusalAssertions = [
  // CLI policy refusals exit nonzero.
  /\.not\.toBe\(0\)/,
  // Store constraints refuse by throwing before a CLI result exists.
  /\.toThrow\(/,
  // Structured refusals pin their error code directly or in the JSON envelope.
  /\.code\)\.toBe\(["'`][A-Z][A-Z0-9_]+["'`]\)|\.toContain\(\s*["'`][^)\n]*code[^)\n]*[A-Z][A-Z0-9_]+[^)\n]*\)/,
  // Claude's hook protocol refuses with a deny decision and a successful hook exit.
  /permissionDecision:\s*["'`]deny["'`]/,
];

test("479 [lint] enforcement registry has a parseable five-cell shape with unique stable ids", async () => {
  const parsed = await registry();
  expect(parsed.headers).toEqual(["id", "Boundary", "Enforced by", "Proof", "Soft-audited"]);
  expect(parsed.rows.length).toBeGreaterThan(0);
  for (const row of parsed.rows) {
    expect(row.cells, row.cells.join(" | ")).toHaveLength(5);
    expect(row.values.id).toMatch(/^B\d+$/);
  }
  const ids = parsed.rows.map((row) => row.values.id);
  expect(new Set(ids).size).toBe(ids.length);
});

test("480 [lint] every enforced registry row cites an existing numbered test", async () => {
  const parsed = await registry();
  const tests = await numberedTests();
  for (const row of enforcingRows(parsed.rows)) {
    const number = citation(row);
    expect(number, `${row.values.id || row.values.Boundary}: missing proof citation`).not.toBeNull();
    const matches = number === null ? [] : tests.get(number) ?? [];
    expect(
      matches.length,
      `${row.values.id}: test ${number} resolves to ${matches.map((match) => match.file).join(", ") || "no file"}`,
    ).toBe(1);
  }
});

test("481 [lint] every enforcement citation contains an attack marker and a refusal assertion", async () => {
  const parsed = await registry();
  const tests = await numberedTests();
  for (const row of enforcingRows(parsed.rows)) {
    const number = citation(row);
    const matches = number === null ? [] : tests.get(number) ?? [];
    const source = matches.length === 1 ? (matches[0] as NumberedTest).source : "";
    expect(
      attackMarkers.some((pattern) => pattern.test(source)),
      `${row.values.id || row.values.Boundary}: test ${number ?? "missing"} has no attack marker`,
    ).toBeTrue();
    expect(
      refusalAssertions.some((pattern) => pattern.test(source)),
      `${row.values.id || row.values.Boundary}: test ${number ?? "missing"} has no refusal assertion`,
    ).toBeTrue();
  }
});

test("482 [lint] soft-audited registry rows make no enforcement claim", async () => {
  const parsed = await registry();
  for (const row of parsed.rows.filter((candidate) => candidate.values["Enforced by"] === "nothing")) {
    expect(row.values.Proof, row.values.id).toBe("soft-audited");
    expect(
      `${row.values.Boundary} ${row.values["Soft-audited"]}`,
      row.values.id,
    ).not.toMatch(/\b(?:enforced|prevents|blocks)\b/i);
  }
});

test("483 [lint] every SLP mechanism token is covered by the registry or a reasoned allowlist", async () => {
  const parsed = await registry();
  const recipe = await readFile(recipePath, "utf8");
  const tokens = new Set(recipe.match(/\b[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+\b|\bPreToolUse\b/g) ?? []);
  const enforcement = parsed.rows.map((row) => row.values["Enforced by"]).join(" ");

  for (const [token, reason] of Object.entries(mechanismAllowlist)) {
    expect(tokens.has(token), `${token}: stale allowlist entry`).toBeTrue();
    expect(reason.trim().length, `${token}: allowlist reason`).toBeGreaterThan(0);
  }
  for (const token of tokens) {
    expect(
      enforcement.includes(token) || token in mechanismAllowlist,
      `${token}: absent from registry enforcement and allowlist`,
    ).toBeTrue();
  }
});
