#!/usr/bin/env bun
// scripts/shape.ts — the cleanup measuring tool for the Hermes-shape cleanup (w732, d130, d131).
//
// Prints the shape table for the current tree with before/now columns, and exits
// non-zero when a d131 cap is broken: a file under src over 1,000 lines, or a
// function, method or arrow body over 150 lines. An allowlist file carries the
// offenders still tolerated, so the execution phases burn the list down entry by
// entry; a stale allowlist entry is itself a failure, so the list cannot rot.
//
// The parse is a real TypeScript parse, not a regex: the tree comes from the
// typescript package's API server (typescript/unstable/async) resolved against
// this repo's tsconfig.json, and the duplicate-body normaliser runs the
// TypeScript scanner over each body.
//
// usage:
//   bun scripts/shape.ts [--allow <path>] [--baseline <path>] [--write-baseline <path>]
//                        [--emit-allow] [--json] [--no-timing] [--timing-runs <n>]
//                        [--root <dir>]
//
// --emit-allow prints an allowlist covering every offender in the current tree,
// so a phase can regenerate its starting list instead of hand-typing it.

import { createScanner, SyntaxKind } from "typescript/unstable/ast";
import { API, fileNameToDocumentURI } from "typescript/unstable/async";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";

// TypeScript 7 ships the Go compiler: the classic `createSourceFile` JS parser is
// gone and some SyntaxKind members were renamed (EndOfFileToken became EndOfFile).
// An undefined member compares equal to nothing, which turns a scan loop into a
// hang, so every kind this tool depends on is checked once, by name, at load.
const REQUIRED_KINDS = [
  "EndOfFile",
  "Identifier",
  "PrivateIdentifier",
  "StringLiteral",
  "NumericLiteral",
  "BigIntLiteral",
  "NoSubstitutionTemplateLiteral",
  "TemplateHead",
  "TemplateMiddle",
  "TemplateTail",
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunction",
  "MethodDeclaration",
  "Constructor",
  "GetAccessor",
  "SetAccessor",
  "ExportKeyword",
  "VariableStatement",
  "ClassDeclaration",
  "InterfaceDeclaration",
  "TypeAliasDeclaration",
  "EnumDeclaration",
  "ModuleDeclaration",
  "ExportDeclaration",
  "ExportAssignment",
  "PropertyAssignment",
  "CallExpression",
] as const;

for (const kind of REQUIRED_KINDS) {
  if ((SyntaxKind as Record<string, unknown>)[kind] === undefined) {
    throw new Error(
      `this TypeScript build has no SyntaxKind.${kind}; scripts/shape.ts needs updating for it`,
    );
  }
}

const FILE_LINE_CAP = 1_000;
const FUNCTION_LINE_CAP = 150;
const DUPLICATE_MIN_LINES = 8;
// Median of 10 is not stable enough to police a 10% regression bound: four
// repeated measurements of 10 spread 7.9%, while four of 30 spread 2.3%
// (NOTES.md, "ready timing"). 30 is the smallest N that leaves the bound real.
const DEFAULT_TIMING_RUNS = 30;

interface FunctionRecord {
  file: string;
  name: string;
  kind: string;
  startLine: number;
  endLine: number;
  lines: number;
  bodyDigest: string | null;
}

interface DuplicateFamily {
  digest: string;
  lines: number;
  members: { file: string; name: string; startLine: number }[];
}

interface Metrics {
  hash: string;
  measuredAt: string;
  srcLines: number;
  srcFiles: number;
  filesOverCap: { file: string; lines: number }[];
  functionsOverCap: FunctionRecord[];
  duplicateFamilies: number;
  duplicateCopies: number;
  duplicates: DuplicateFamily[];
  exportedNames: number;
  listMedianMs: number | null;
  listMinMs: number | null;
  listMaxMs: number | null;
  listRuns: number;
}

interface AllowEntry {
  type: "file" | "fn";
  file: string;
  name: string;
  line: number;
  raw: string;
}

// ---------------------------------------------------------------- arguments

function argument(flag: string): string | undefined {
  const index = process.argv.indexOf(flag);
  if (index === -1) return undefined;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${flag} needs a value`);
  }
  return value;
}

const flags = new Set(process.argv.slice(2).filter((token) => token.startsWith("--")));
const root = resolve(argument("--root") ?? process.cwd());
const asJson = flags.has("--json");
const withTiming = !flags.has("--no-timing");
const timingRuns = Number(argument("--timing-runs") ?? DEFAULT_TIMING_RUNS);
const allowPath = argument("--allow");
const baselinePath = argument("--baseline");
const writeBaselinePath = argument("--write-baseline");
const emitAllow = flags.has("--emit-allow");

// ------------------------------------------------------------------- source

async function sourceFiles(): Promise<string[]> {
  const glob = new Bun.Glob("src/**/*.ts");
  const found: string[] = [];
  for await (const file of glob.scan({ cwd: root })) found.push(file);
  return found.filter((file) => !file.endsWith(".test.ts")).sort();
}

// `wc -l` semantics: a trailing newline closes the last line rather than opening one.
function countLines(text: string): number {
  if (text === "") return 0;
  const parts = text.split("\n");
  return parts.at(-1) === "" ? parts.length - 1 : parts.length;
}

// --------------------------------------------------------------------- AST

const FUNCTION_KINDS = new Map<number, string>([
  [SyntaxKind.FunctionDeclaration, "function"],
  [SyntaxKind.FunctionExpression, "function expression"],
  [SyntaxKind.ArrowFunction, "arrow"],
  [SyntaxKind.MethodDeclaration, "method"],
  [SyntaxKind.Constructor, "constructor"],
  [SyntaxKind.GetAccessor, "getter"],
  [SyntaxKind.SetAccessor, "setter"],
]);

// An anonymous function borrows the name of whatever it is bound to, and failing
// that the name of the nearest named function around it, as `outer>arrow`. The
// key must not contain a line number: an allowlist entry has to survive the very
// refactoring it is tracking, and line numbers churn on every edit.
function boundName(node: any): string | undefined {
  const own = node.name?.text ?? node.name?.getText?.();
  if (own) return String(own);
  const parent = node.parent;
  if (!parent) return undefined;
  if (
    parent.kind === SyntaxKind.VariableDeclaration ||
    parent.kind === SyntaxKind.PropertyAssignment ||
    parent.kind === SyntaxKind.PropertyDeclaration
  ) {
    const bound = parent.name?.text ?? parent.name?.getText?.();
    if (bound) return String(bound);
  }
  return undefined;
}

function nameOf(node: any): string {
  const own = boundName(node);
  if (own) return own;
  for (let cursor = node.parent; cursor; cursor = cursor.parent) {
    if (!FUNCTION_KINDS.has(cursor.kind)) continue;
    const outer = boundName(cursor);
    if (outer) return `${outer}>${FUNCTION_KINDS.get(node.kind) ?? "fn"}`;
  }
  return `<${FUNCTION_KINDS.get(node.kind) ?? "anonymous"}>`;
}

// A body reduced to a canonical token stream: every identifier and every literal
// becomes its kind, so two helpers that differ only in the names and constants
// they use collapse to one digest, and comments never reach it.
//
// The literal ranges are masked from the AST first, then the TypeScript scanner
// drops comments and whitespace over the masked text. The masking is not an
// optimisation: a standalone scanner has no parser context, so it reads a regex
// literal's leading slash as a division operator and then stalls on the
// unterminated remainder. Masking regex literals out of the text before it is
// scanned is what keeps the scan honest.
const MASKED_KINDS = new Map<number, string>([
  [SyntaxKind.Identifier, "ID"],
  [SyntaxKind.PrivateIdentifier, "ID"],
  [SyntaxKind.StringLiteral, "STR"],
  [SyntaxKind.NumericLiteral, "NUM"],
  [SyntaxKind.BigIntLiteral, "NUM"],
  [SyntaxKind.RegularExpressionLiteral, "RE"],
  [SyntaxKind.NoSubstitutionTemplateLiteral, "TPL"],
  [SyntaxKind.TemplateHead, "TPL"],
  [SyntaxKind.TemplateMiddle, "TPL"],
  [SyntaxKind.TemplateTail, "TPL"],
]);

function maskedText(body: any): string {
  const base = body.getStart();
  const text: string = body.getText();
  const spans: { start: number; end: number; token: string }[] = [];
  const collect = (node: any): void => {
    const token = MASKED_KINDS.get(node.kind);
    if (token) {
      spans.push({ start: node.getStart() - base, end: node.getEnd() - base, token });
    }
    node.forEachChild(collect);
  };
  collect(body);
  spans.sort((left, right) => left.start - right.start);

  let out = "";
  let cursor = 0;
  for (const span of spans) {
    if (span.start < cursor || span.start < 0 || span.end > text.length) continue;
    out += text.slice(cursor, span.start);
    out += ` ${span.token} `;
    cursor = span.end;
  }
  return out + text.slice(cursor);
}

function normalizedBody(body: any): string {
  const text = maskedText(body);
  const scanner = createScanner(true, undefined, text);
  const tokens: string[] = [];
  let previousEnd = -1;
  for (;;) {
    const token = scanner.scan();
    // The scanner keeps returning EndOfFile once it is done, so the loop stops on
    // it. A token that does not advance the cursor would otherwise spin forever,
    // and silently truncating the stream would make two different bodies compare
    // equal, so it is an error rather than a break.
    if (token === SyntaxKind.EndOfFile) break;
    const end = scanner.getTokenEnd();
    if (end <= previousEnd) {
      throw new Error(
        `the TypeScript scanner stopped advancing at offset ${end}; the masked body cannot be normalised`,
      );
    }
    previousEnd = end;
    tokens.push(token === SyntaxKind.Identifier ? scanner.getTokenText() : String(token));
  }
  return tokens.join(" ");
}

function digestOf(text: string): string {
  return Bun.hash(text).toString(16);
}

function exportedNamesOf(sourceFile: any): string[] {
  const names: string[] = [];
  const declared = (node: any): void => {
    switch (node.kind) {
      case SyntaxKind.FunctionDeclaration:
      case SyntaxKind.ClassDeclaration:
      case SyntaxKind.InterfaceDeclaration:
      case SyntaxKind.TypeAliasDeclaration:
      case SyntaxKind.EnumDeclaration:
      case SyntaxKind.ModuleDeclaration:
        if (node.name?.text) names.push(String(node.name.text));
        return;
      case SyntaxKind.VariableStatement:
        for (const declaration of node.declarationList?.declarations ?? []) {
          if (declaration.name?.text) names.push(String(declaration.name.text));
        }
        return;
      default:
        return;
    }
  };
  for (const statement of sourceFile.statements ?? []) {
    const modifiers: any[] = statement.modifiers ?? [];
    const exported = modifiers.some((modifier) => modifier.kind === SyntaxKind.ExportKeyword);
    if (exported) {
      declared(statement);
      continue;
    }
    if (statement.kind === SyntaxKind.ExportDeclaration) {
      const clause = statement.exportClause;
      for (const element of clause?.elements ?? []) {
        if (element.name?.text) names.push(String(element.name.text));
      }
      continue;
    }
    if (statement.kind === SyntaxKind.ExportAssignment) names.push("default");
  }
  return names;
}

async function measureTree(): Promise<{
  functions: FunctionRecord[];
  exported: Set<string>;
  files: { file: string; lines: number }[];
  srcLines: number;
}> {
  const files = await sourceFiles();
  if (files.length === 0) throw new Error(`no source files under ${join(root, "src")}`);

  const api = new API({ cwd: root });
  const anchor = resolve(root, files[0]!);
  const snapshot: any = await api.updateSnapshot({
    openFiles: [{ uri: fileNameToDocumentURI(anchor) }],
  } as any);
  const [project] = await snapshot.getProjects();
  if (!project) throw new Error("the typescript API returned no project for this tree");

  const functions: FunctionRecord[] = [];
  const exported = new Set<string>();
  const sizes: { file: string; lines: number }[] = [];
  let srcLines = 0;

  for (const file of files) {
    const absolute = resolve(root, file);
    const text = await readFile(absolute, "utf8");
    const lines = countLines(text);
    srcLines += lines;
    sizes.push({ file, lines });

    const sourceFile: any = await project.program.getSourceFile({
      uri: fileNameToDocumentURI(absolute),
    });
    if (!sourceFile) throw new Error(`the typescript API did not return a tree for ${file}`);

    for (const name of exportedNamesOf(sourceFile)) exported.add(`${file}:${name}`);

    const visit = (node: any): void => {
      const kind = FUNCTION_KINDS.get(node.kind);
      if (kind && node.body) {
        const start = sourceFile.getLineAndCharacterOfPosition(node.body.getStart());
        const end = sourceFile.getLineAndCharacterOfPosition(node.body.getEnd());
        const span = end.line - start.line + 1;
        functions.push({
          file,
          name: nameOf(node),
          kind,
          startLine: start.line + 1,
          endLine: end.line + 1,
          lines: span,
          bodyDigest: span >= DUPLICATE_MIN_LINES ? digestOf(normalizedBody(node.body)) : null,
        });
      }
      node.forEachChild(visit);
    };
    sourceFile.forEachChild(visit);
  }

  await api.close();
  return { functions, exported, files: sizes, srcLines };
}

// ------------------------------------------------------------------ timing

// The hook runs the CLI on every prompt, so the cost that matters is a cold
// start in a cwd that is not this repo. The first run creates the scratch store
// and is discarded; the median of the rest is the figure.
async function measureList(runs: number): Promise<{
  median: number | null;
  min: number | null;
  max: number | null;
}> {
  const entry = join(root, "bin", "maestro.ts");
  if (!existsSync(entry)) return { median: null, min: null, max: null };
  const scratch = await mkdtemp(join(tmpdir(), "maestro-shape-"));
  try {
    const run = async (): Promise<number> => {
      const started = Bun.nanoseconds();
      const process_ = Bun.spawn(["bun", entry, "work", "list"], {
        cwd: scratch,
        stdout: "ignore",
        stderr: "ignore",
        env: { ...process.env, MAESTRO_READ_ONLY: undefined },
      });
      await process_.exited;
      return (Bun.nanoseconds() - started) / 1_000_000;
    };
    await run();
    const samples: number[] = [];
    for (let index = 0; index < runs; index += 1) samples.push(await run());
    samples.sort((left, right) => left - right);
    const middle = Math.floor(samples.length / 2);
    const median =
      samples.length % 2 === 0
        ? (samples[middle - 1]! + samples[middle]!) / 2
        : samples[middle]!;
    return { median, min: samples[0]!, max: samples.at(-1)! };
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

// --------------------------------------------------------------- allowlist

async function readAllowlist(path: string): Promise<AllowEntry[]> {
  const text = await readFile(resolve(root, path), "utf8");
  const entries: AllowEntry[] = [];
  text.split("\n").forEach((raw, index) => {
    const line = raw.trim();
    if (line === "" || line.startsWith("#")) return;
    const [type, file, ...rest] = line.split(/\s+/);
    if (type !== "file" && type !== "fn") {
      throw new Error(`${path}:${index + 1}: expected "file <path>" or "fn <path> <name>", got: ${line}`);
    }
    if (!file) throw new Error(`${path}:${index + 1}: missing path`);
    if (type === "fn" && rest.length === 0) {
      throw new Error(`${path}:${index + 1}: "fn ${file}" is missing the function name`);
    }
    entries.push({ type, file, name: rest.join(" "), line: index + 1, raw: line });
  });
  return entries;
}

// Each entry pardons exactly one offender, so an entry cannot quietly cover a
// second copy that appears later, and an entry that pardons nothing is stale.
function applyAllowlist(
  offenders: { key: string; describe: string }[],
  entries: AllowEntry[],
  type: "file" | "fn",
): { remaining: { key: string; describe: string }[]; stale: AllowEntry[] } {
  const budget = new Map<string, AllowEntry[]>();
  for (const entry of entries.filter((candidate) => candidate.type === type)) {
    const key = type === "file" ? entry.file : `${entry.file}:${entry.name}`;
    budget.set(key, [...(budget.get(key) ?? []), entry]);
  }
  const remaining: { key: string; describe: string }[] = [];
  for (const offender of offenders) {
    const pardons = budget.get(offender.key);
    if (pardons && pardons.length > 0) {
      pardons.shift();
      continue;
    }
    remaining.push(offender);
  }
  const stale = [...budget.values()].flat();
  return { remaining, stale };
}

// ------------------------------------------------------------------ render

function pad(text: string, width: number): string {
  return text.padEnd(width);
}

function formatMs(value: number | null): string {
  return value === null ? "-" : `${value.toFixed(1)} ms`;
}

function renderTable(now: Metrics, before: Metrics | null): string {
  const rows: [string, string, string][] = [
    ["src lines (non-test)", before ? String(before.srcLines) : "-", String(now.srcLines)],
    ["src files", before ? String(before.srcFiles) : "-", String(now.srcFiles)],
    [
      `files > ${FILE_LINE_CAP} lines`,
      before ? String(before.filesOverCap.length) : "-",
      String(now.filesOverCap.length),
    ],
    [
      `functions > ${FUNCTION_LINE_CAP} lines`,
      before ? String(before.functionsOverCap.length) : "-",
      String(now.functionsOverCap.length),
    ],
    [
      "duplicated helper families",
      before ? String(before.duplicateFamilies) : "-",
      String(now.duplicateFamilies),
    ],
    [
      "duplicated helper copies",
      before ? String(before.duplicateCopies) : "-",
      String(now.duplicateCopies),
    ],
    ["exported names", before ? String(before.exportedNames) : "-", String(now.exportedNames)],
    [
      "maestro work list (median)",
      before ? formatMs(before.listMedianMs) : "-",
      formatMs(now.listMedianMs),
    ],
  ];
  const widths = [
    Math.max(...rows.map(([label]) => label.length), "metric".length),
    Math.max(...rows.map(([, value]) => value.length), "before".length),
    Math.max(...rows.map(([, , value]) => value.length), "now".length),
  ];
  const line = (cells: [string, string, string]): string =>
    `| ${pad(cells[0], widths[0]!)} | ${pad(cells[1], widths[1]!)} | ${pad(cells[2], widths[2]!)} |`;
  const rule = `|${"-".repeat(widths[0]! + 2)}|${"-".repeat(widths[1]! + 2)}|${"-".repeat(widths[2]! + 2)}|`;
  return [line(["metric", "before", "now"]), rule, ...rows.map(line)].join("\n");
}

// -------------------------------------------------------------------- main

async function main(): Promise<number> {
  const hash = (await Bun.$`git -C ${root} rev-parse HEAD`.quiet().nothrow()).stdout
    .toString()
    .trim();
  const { functions, exported, files, srcLines } = await measureTree();

  const byDigest = new Map<string, FunctionRecord[]>();
  for (const record of functions) {
    if (!record.bodyDigest) continue;
    byDigest.set(record.bodyDigest, [...(byDigest.get(record.bodyDigest) ?? []), record]);
  }
  const duplicates: DuplicateFamily[] = [...byDigest.entries()]
    .filter(([, members]) => members.length > 1)
    .map(([digest, members]) => ({
      digest,
      lines: members[0]!.lines,
      members: members.map((member) => ({
        file: member.file,
        name: member.name,
        startLine: member.startLine,
      })),
    }))
    .sort((left, right) => right.members.length * right.lines - left.members.length * left.lines);

  const timing = withTiming
    ? await measureList(timingRuns)
    : { median: null, min: null, max: null };

  const now: Metrics = {
    hash,
    measuredAt: new Date().toISOString(),
    srcLines,
    srcFiles: files.length,
    filesOverCap: files
      .filter((file) => file.lines > FILE_LINE_CAP)
      .sort((left, right) => right.lines - left.lines),
    functionsOverCap: functions
      .filter((record) => record.lines > FUNCTION_LINE_CAP)
      .sort((left, right) => right.lines - left.lines),
    duplicateFamilies: duplicates.length,
    duplicateCopies: duplicates.reduce((total, family) => total + family.members.length - 1, 0),
    duplicates,
    exportedNames: exported.size,
    listMedianMs: timing.median,
    listMinMs: timing.min,
    listMaxMs: timing.max,
    listRuns: withTiming ? timingRuns : 0,
  };

  if (emitAllow) {
    const lines = [
      "# scripts/shape.ts allowlist: offenders tolerated until their phase lands.",
      "# Each line pardons exactly one offender; delete the line when it is fixed.",
      "# A line that pardons nothing fails the gate, so this list cannot rot.",
      `# generated from ${hash || "(unknown)"} on ${now.measuredAt}`,
      "",
      `# ${now.filesOverCap.length} files over ${FILE_LINE_CAP} lines`,
      ...now.filesOverCap.map((file) => `file ${file.file}`),
      "",
      `# ${now.functionsOverCap.length} function bodies over ${FUNCTION_LINE_CAP} lines`,
      ...now.functionsOverCap.map((record) => `fn ${record.file} ${record.name}`),
      "",
    ];
    console.log(lines.join("\n"));
    return 0;
  }

  if (writeBaselinePath) {
    await writeFile(resolve(root, writeBaselinePath), `${JSON.stringify(now, null, 2)}\n`);
  }

  let before: Metrics | null = null;
  if (baselinePath) {
    const path = resolve(root, baselinePath);
    if (existsSync(path)) before = JSON.parse(await readFile(path, "utf8")) as Metrics;
  }

  const allowEntries = allowPath ? await readAllowlist(allowPath) : [];
  const fileOffenders = now.filesOverCap.map((file) => ({
    key: file.file,
    describe: `${file.file} is ${file.lines} lines (cap ${FILE_LINE_CAP})`,
  }));
  const functionOffenders = now.functionsOverCap.map((record) => ({
    key: `${record.file}:${record.name}`,
    describe: `${record.file}:${record.startLine} ${record.kind} ${record.name} body is ${record.lines} lines (cap ${FUNCTION_LINE_CAP})`,
  }));
  const fileResult = applyAllowlist(fileOffenders, allowEntries, "file");
  const functionResult = applyAllowlist(functionOffenders, allowEntries, "fn");
  const stale = [...fileResult.stale, ...functionResult.stale];
  const failures = [...fileResult.remaining, ...functionResult.remaining];

  if (asJson) {
    console.log(
      JSON.stringify({
        ok: failures.length === 0 && stale.length === 0,
        now,
        before,
        violations: failures.map((failure) => failure.describe),
        staleAllowlist: stale.map((entry) => `${allowPath}:${entry.line}: ${entry.raw}`),
      }),
    );
  } else {
    console.log(renderTable(now, before));
    console.log("");
    console.log(`hash: ${hash || "(unknown)"}`);
    if (now.listMedianMs !== null) {
      console.log(
        `maestro work list: median ${formatMs(now.listMedianMs)} over ${now.listRuns} runs ` +
          `(min ${formatMs(now.listMinMs)}, max ${formatMs(now.listMaxMs)})`,
      );
    }
    if (now.filesOverCap.length > 0) {
      console.log(`\nfiles over ${FILE_LINE_CAP} lines:`);
      for (const file of now.filesOverCap) console.log(`  ${file.lines.toString().padStart(6)}  ${file.file}`);
    }
    if (now.functionsOverCap.length > 0) {
      console.log(`\nfunction bodies over ${FUNCTION_LINE_CAP} lines:`);
      for (const record of now.functionsOverCap) {
        console.log(
          `  ${record.lines.toString().padStart(6)}  ${record.file}:${record.startLine} ${record.kind} ${record.name}`,
        );
      }
    }
    if (now.duplicates.length > 0) {
      console.log(
        `\nduplicated helper families (identical token streams, body >= ${DUPLICATE_MIN_LINES} lines): ${now.duplicateFamilies} families, ${now.duplicateCopies} removable copies`,
      );
      for (const family of now.duplicates) {
        console.log(`  ${family.lines} lines x ${family.members.length}:`);
        for (const member of family.members) {
          console.log(`      ${member.file}:${member.startLine} ${member.name}`);
        }
      }
    }
    if (stale.length > 0) {
      console.log("\nstale allowlist entries (delete these lines):");
      for (const entry of stale) console.log(`  ${allowPath}:${entry.line}: ${entry.raw}`);
    }
    if (failures.length > 0) {
      console.log("\nd131 cap violations:");
      for (const failure of failures) console.log(`  ${failure.describe}`);
    }
  }

  return failures.length === 0 && stale.length === 0 ? 0 : 1;
}

process.exit(await main());
