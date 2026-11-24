#!/usr/bin/env bun
// The approved list and the pin are read from the base ref, so a change cannot approve itself (d914).
import { Database } from "bun:sqlite";
import { cp, mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const listPath = "tests/differential/approved-changes.txt";
const pinPath = "tests/differential/old-commit";
const corpusPath = "tests/differential/corpus";
const kinds = ["gone", "flag-gone", "output", "any", "drop-line"] as const;

type Kind = (typeof kinds)[number];

interface Rule {
  kind: Kind;
  prefix: string[];
  flag?: string;
  flagTakesValue?: boolean;
  pattern?: RegExp;
  when?: RegExp;
  file?: string;
  text: string;
  used: number;
}

interface Step {
  actor: string;
  argv: string[];
  source: string;
}

interface Outcome {
  exitCode: number;
  stdout: string;
  stderr: string;
}

interface Side {
  tree: string;
  root: string;
}

function option(argv: string[], name: string): string | undefined {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : undefined;
}

async function capture(command: string[], cwd: string): Promise<Outcome> {
  const child = Bun.spawn(command, { cwd, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { exitCode, stdout, stderr };
}

async function git(root: string, args: string[]): Promise<string> {
  const result = await capture(["git", ...args], root);
  if (result.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${result.stderr.trim()}`);
  return result.stdout;
}

async function fromBase(root: string, base: string | undefined, path: string): Promise<string> {
  if (base === undefined) return readFile(join(root, path), "utf8");
  return git(root, ["show", `${base}:${path}`]);
}

function splitWords(line: string): string[] {
  const words: string[] = [];
  let current = "";
  let quote = "";
  let open = false;
  for (const char of line) {
    if (quote) {
      if (char === quote) quote = "";
      else current += char;
    } else if (char === '"' || char === "'") {
      quote = char;
      open = true;
    } else if (char === " " || char === "\t") {
      if (open || current) words.push(current);
      current = "";
      open = false;
    } else {
      current += char;
    }
  }
  if (quote) throw new Error(`unterminated quote: ${line}`);
  if (open || current) words.push(current);
  return words;
}

function expression(source: string, line: string): RegExp {
  if (!source.startsWith("/") || !source.endsWith("/") || source.length < 3) {
    throw new Error(`approved list: expected /regex/ in: ${line}`);
  }
  return new RegExp(source.slice(1, -1));
}

function parseRules(text: string): Rule[] {
  const rules: Rule[] = [];
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const hash = line.indexOf(" # ");
    if (hash < 0 || !/\bd\d+\b/.test(line.slice(hash))) {
      throw new Error(`approved list: a rule must end with "# d<id>: reason": ${line}`);
    }
    let body = line.slice(0, hash);
    const rule: Rule = { kind: body.split(" ")[0] as Kind, prefix: [], text: line, used: 0 };
    if (!kinds.includes(rule.kind)) throw new Error(`approved list: unknown rule kind: ${line}`);
    const take = (pattern: RegExp): string | undefined => {
      const found = pattern.exec(body);
      if (found) body = body.slice(0, found.index);
      return found?.[1];
    };
    rule.file = take(/ in (\S+\.seq)$/);
    const when = take(/ when (\/.+\/)$/);
    if (when) rule.when = expression(when, line);
    if (rule.kind === "drop-line") rule.pattern = expression(take(/ (\/.+\/)$/) ?? "", line);
    const words = splitWords(body).slice(1);
    if (rule.kind === "flag-gone") {
      const last = words.pop() ?? "";
      rule.flagTakesValue = last === "value";
      rule.flag = rule.flagTakesValue ? (words.pop() ?? "") : last;
      if (!rule.flag.startsWith("--")) throw new Error(`approved list: flag-gone needs a --flag: ${line}`);
    }
    if (words.length === 0) throw new Error(`approved list: a rule needs a command prefix: ${line}`);
    rule.prefix = words;
    rules.push(rule);
  }
  return rules;
}

function matches(rule: Rule, step: Step): boolean {
  const argv = step.argv;
  if (rule.file !== undefined && !step.source.startsWith(`${rule.file}:`)) return false;
  if (rule.prefix.length > argv.length) return false;
  if (!rule.prefix.every((word, index) => word === "*" || word === argv[index])) return false;
  return rule.flag === undefined || argv.includes(rule.flag);
}

function parseCorpus(name: string, text: string): Step[] {
  const steps: Step[] = [];
  for (const [index, raw] of text.split("\n").entries()) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const words = splitWords(line);
    const actor = words[0]?.startsWith("@") ? (words.shift() as string).slice(1) : "a";
    steps.push({ actor, argv: words, source: `${name}:${index + 1}` });
  }
  return steps;
}

async function run(side: Side, step: Step, argv: string[]): Promise<Outcome> {
  const environment: Record<string, string> = {
    HOME: join(side.root, "home"),
    PATH: process.env.PATH ?? "",
    TMPDIR: process.env.TMPDIR ?? tmpdir(),
    MAESTRO_SESSION_ID: `differential-${step.actor}`,
    MAESTRO_SESSION_PID: String(process.pid),
  };
  const child = Bun.spawn([process.execPath, join(side.tree, "bin", "maestro.ts"), ...argv], {
    cwd: join(side.root, "repo"),
    env: environment,
    stdin: new TextEncoder().encode(""),
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { exitCode, stdout, stderr };
}

function normalize(text: string, side: Side, drops: RegExp[]): string {
  let body = text;
  const paths = [...new Set([side.root, realpathSync(side.root), side.tree, realpathSync(side.tree)])];
  for (const path of paths.sort((left, right) => right.length - left.length)) {
    body = body.split(path).join("<PATH>");
  }
  body = body
    .replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?Z/g, "<TIME>")
    .replace(/\d{4}-\d\d-\d\d/g, "<DATE>")
    .replace(new RegExp(`\\b${process.pid}\\b`, "g"), "<PID>")
    .replace(/\b\d+(\.\d+)? ?(ms|s)\b/g, "<DURATION>")
    .replace(/\bmaestro \d+\.\d+\.\d+\b/g, "maestro <VERSION>");
  if (drops.length === 0) return body;
  return body
    .split("\n")
    .filter((line) => !drops.some((drop) => drop.test(line)))
    .join("\n");
}

async function freshSide(tree: string, scratch: string, label: string): Promise<Side> {
  const root = await mkdtemp(join(scratch, `${label}-`));
  await mkdir(join(root, "home"), { recursive: true });
  await mkdir(join(root, "repo", ".maestro"), { recursive: true });
  return { tree, root };
}

function excerpt(text: string): string {
  const lines = text.trimEnd().split("\n");
  return lines.slice(0, 12).join("\n      ") + (lines.length > 12 ? `\n      ... ${lines.length - 12} more lines` : "");
}

async function compareStep(
  rules: Rule[],
  oldSide: Side,
  newSide: Side,
  step: Step,
  strict: boolean,
): Promise<string | null> {
  const applicable = rules.filter((rule) => matches(rule, step));
  const oldResult = await run(oldSide, step, step.argv);
  let newResult = await run(newSide, step, step.argv);
  const flagRules = applicable.filter((candidate) => candidate.kind === "flag-gone");
  for (const rule of flagRules) rule.used += 1;
  const rejected = /UNKNOWN_FLAG/.test(newResult.stdout + newResult.stderr);
  if (flagRules.length > 0 && rejected) {
    let stripped = step.argv;
    for (const rule of flagRules) {
      const at = stripped.indexOf(rule.flag as string);
      stripped = [...stripped.slice(0, at), ...stripped.slice(at + (rule.flagTakesValue ? 2 : 1))];
    }
    newResult = await run(newSide, step, stripped);
  } else if (flagRules.length > 0 && strict) {
    return `${step.source}: maestro ${step.argv.join(" ")}\n    listed as flag-gone, yet the new tree still accepts the flag\n    rule: ${(flagRules[0] as Rule).text}`;
  }
  const oldRaw = oldResult.stdout + oldResult.stderr;
  const live = applicable.filter((rule) => rule.when === undefined || rule.when.test(oldRaw));
  const drops = live.filter((rule) => rule.kind === "drop-line").map((rule) => rule.pattern as RegExp);
  for (const rule of live.filter((candidate) => candidate.kind !== "flag-gone")) rule.used += 1;

  const before = { exit: oldResult.exitCode, out: normalize(oldResult.stdout, oldSide, drops), err: normalize(oldResult.stderr, oldSide, drops) };
  const after = { exit: newResult.exitCode, out: normalize(newResult.stdout, newSide, drops), err: normalize(newResult.stderr, newSide, drops) };
  if (process.argv.includes("--trace")) {
    console.log(`${step.source} [${before.exit}/${after.exit}] maestro ${step.argv.join(" ")} => ${(before.out + before.err).split("\n")[0]?.slice(0, 110)}`);
  }
  const gone = live.find((rule) => rule.kind === "gone");
  if (gone) {
    if (strict && after.exit === 0) return `${step.source}: maestro ${step.argv.join(" ")}\n    listed as gone, yet the new tree still accepts it\n    rule: ${gone.text}`;
    return null;
  }
  if (live.some((rule) => rule.kind === "any")) return null;
  const sameText = before.out === after.out && before.err === after.err;
  if (before.exit === after.exit && (sameText || live.some((rule) => rule.kind === "output"))) return null;
  return [
    `${step.source}: maestro ${step.argv.join(" ")}`,
    `    old exit ${before.exit}, new exit ${after.exit}`,
    `    old: ${excerpt(before.out + before.err)}`,
    `    new: ${excerpt(after.out + after.err)}`,
  ].join("\n");
}

async function compareTrees(options: {
  root: string;
  oldTree: string;
  newTree: string;
  rules: Rule[];
  scratch: string;
  strict: boolean;
}): Promise<string[]> {
  const failures: string[] = [];
  const directory = join(options.root, corpusPath);
  const names = (await readdir(directory)).filter((name) => name.endsWith(".seq")).sort();
  await Promise.all(
    names.map(async (name) => {
      const steps = parseCorpus(name, await readFile(join(directory, name), "utf8"));
      const oldSide = await freshSide(options.oldTree, options.scratch, "old");
      const newSide = await freshSide(options.newTree, options.scratch, "new");
      for (const step of steps) {
        const failure = await compareStep(options.rules, oldSide, newSide, step, options.strict);
        if (failure) failures.push(failure);
      }
      failures.push(...(await checkOldStore(oldSide, options, name)));
    }),
  );
  return failures.sort();
}

function census(path: string): Map<string, number> {
  const database = new Database(path, { readwrite: true });
  try {
    const tables = database
      .query<{ name: string; sql: string | null }, []>("select name, sql from sqlite_master where type = 'table' and name not like 'sqlite_%'")
      .all();
    const virtual = tables.filter((table) => /^create virtual table/i.test(table.sql ?? "")).map((table) => table.name);
    const counted = tables.filter((table) => !virtual.some((name) => table.name.startsWith(`${name}_`)));
    return new Map(
      counted.map((table) => [
        table.name,
        (database.query<{ count: number }, []>(`select count(*) as count from "${table.name}"`).get() as { count: number }).count,
      ]),
    );
  } finally {
    database.close();
  }
}

function sampledIds(path: string, table: string, limit: number): string[] {
  const database = new Database(path, { readwrite: true });
  try {
    const exists = database.query("select 1 from sqlite_master where type = 'table' and name = ?").get(table);
    if (!exists) return [];
    const ids = database.query<{ id: string }, []>(`select id from "${table}" order by rowid`).all().map((row) => row.id);
    const stride = Math.max(1, Math.floor(ids.length / limit));
    return ids.filter((_, index) => index % stride === 0).slice(0, limit);
  } finally {
    database.close();
  }
}

async function checkStore(store: string, oldSide: Side, newSide: Side, rules: Rule[], label: string): Promise<string[]> {
  const failures: string[] = [];
  const before = census(store);
  const reads: string[][] = [["work", "list"], ["decision", "list"], ["bundle", "list"], ["lesson", "list"], ["term", "list"], ["search", "the"]];
  for (const id of sampledIds(store, "work", 25)) reads.push(["work", "show", id]);
  for (const id of sampledIds(store, "decisions", 25)) reads.push(["decision", "show", id]);
  for (const id of sampledIds(store, "terms", 10)) reads.push(["term", "show", id]);
  for (const id of sampledIds(store, "lessons", 10)) reads.push(["lesson", "show", id]);
  for (const argv of reads) {
    const failure = await compareStep(rules, oldSide, newSide, { actor: "reader", argv, source: label }, false);
    if (failure) failures.push(failure);
  }
  const after = census(join(newSide.root, "repo", ".maestro", "maestro.db"));
  for (const [table, count] of before) {
    const now = after.get(table);
    if (now === undefined) failures.push(`${label}: table ${table} is gone after the new tree opened the store`);
    else if (now < count) failures.push(`${label}: table ${table} lost rows (${count} -> ${now})`);
  }
  return failures;
}

async function checkOldStore(
  oldSide: Side,
  options: { rules: Rule[]; scratch: string; oldTree: string; newTree: string },
  name: string,
): Promise<string[]> {
  const store = join(oldSide.root, "repo", ".maestro", "maestro.db");
  if (!(await Bun.file(store).exists())) return [];
  const oldCopy = await freshSide(options.oldTree, options.scratch, "old-store");
  const newCopy = await freshSide(options.newTree, options.scratch, "new-store");
  for (const copy of [oldCopy, newCopy]) {
    await rm(join(copy.root, "repo"), { recursive: true });
    await cp(join(oldSide.root, "repo"), join(copy.root, "repo"), { recursive: true });
    await cp(join(oldSide.root, "home"), join(copy.root, "home"), { recursive: true });
  }
  return checkStore(store, oldCopy, newCopy, options.rules, `${name}:old-built store`);
}

async function checkRealStores(options: { stores: string[]; oldTree: string; newTree: string; rules: Rule[]; scratch: string }): Promise<string[]> {
  const failures: string[] = [];
  for (const [index, store] of options.stores.entries()) {
    const oldSide = await freshSide(options.oldTree, options.scratch, "old-real");
    const newSide = await freshSide(options.newTree, options.scratch, "new-real");
    const snapshot = join(options.scratch, `store-${index}.db`);
    const backup = await capture(["sqlite3", "-readonly", resolve(store), `.backup '${snapshot}'`], options.scratch);
    if (backup.exitCode !== 0) {
      failures.push(`store ${index}: backup failed: ${backup.stderr.trim()}`);
      continue;
    }
    for (const side of [oldSide, newSide]) await cp(snapshot, join(side.root, "repo", ".maestro", "maestro.db"));
    const found = await checkStore(snapshot, oldSide, newSide, options.rules, `store ${index}`);
    console.log(`store ${index}: ${[...census(snapshot).values()].reduce((sum, count) => sum + count, 0)} rows in ${census(snapshot).size} tables, ${found.length} failures`);
    failures.push(...found);
  }
  return failures;
}

async function materialize(root: string, commit: string, scratch: string): Promise<string> {
  const tree = join(scratch, "old-tree");
  await mkdir(tree, { recursive: true });
  const archive = join(scratch, "old-tree.tar");
  await git(root, ["archive", "--format=tar", "-o", archive, commit]);
  const unpack = await capture(["tar", "-xf", archive, "-C", tree], scratch);
  if (unpack.exitCode !== 0) throw new Error(`tar: ${unpack.stderr.trim()}`);
  return tree;
}

async function selfTest(root: string, oldTree: string, rules: Rule[], scratch: string): Promise<string[]> {
  const problems: string[] = [];
  for (const round of [1, 2]) {
    const same = await compareTrees({ root, oldTree, newTree: oldTree, rules: [], scratch, strict: false });
    if (same.length > 0) problems.push(`self-test round ${round}: the old tree differs from itself in ${same.length} steps:\n${same.slice(0, 5).join("\n")}`);
  }
  const planted = join(scratch, "planted-tree");
  await cp(oldTree, planted, { recursive: true });
  const target = join(planted, "src", "plugins", "work.ts");
  const source = await readFile(target, "utf8");
  if (!source.includes("started by")) problems.push("self-test: the planted change found nothing to change in work.ts");
  await writeFile(target, source.split("started by").join("begun by"));
  const caught = await compareTrees({ root, oldTree, newTree: planted, rules, scratch, strict: false });
  if (caught.length === 0) problems.push("self-test: a planted change to the work start line went unnoticed");
  return problems;
}

const argv = process.argv.slice(2);
const root = resolve(option(argv, "--root") ?? join(import.meta.dir, ".."));
const base = option(argv, "--base");
const scratch = await mkdtemp(join(tmpdir(), "maestro-differential-"));
let failures: string[] = [];
try {
  const rules = parseRules(await fromBase(root, base, listPath));
  const pin = (await fromBase(root, base, pinPath)).trim();
  const oldTree = await materialize(root, pin, scratch);
  const storesAt = argv.indexOf("--stores");
  if (argv.includes("--self-test")) {
    failures = await selfTest(root, oldTree, rules, scratch);
  } else if (storesAt >= 0) {
    const rest = argv.slice(storesAt + 1);
    const end = rest.findIndex((word) => word.startsWith("--"));
    const stores = end < 0 ? rest : rest.slice(0, end);
    failures = await checkRealStores({ stores, oldTree, newTree: root, rules, scratch });
  } else {
    failures = await compareTrees({ root, oldTree, newTree: root, rules, scratch, strict: argv.includes("--strict") });
    for (const rule of rules.filter((candidate) => candidate.used === 0)) {
      failures.push(`approved rule matches no corpus step, so nothing exercises it: ${rule.text}`);
    }
  }
  console.log(`old ${pin.slice(0, 8)}, list from ${base ?? "the working tree"}, ${rules.length} rules`);
} finally {
  await rm(scratch, { recursive: true, force: true });
}
for (const failure of failures) console.log(failure);
console.log(failures.length === 0 ? "differential: no unapproved difference" : `differential: ${failures.length} unapproved`);
process.exit(failures.length === 0 ? 0 : 1);
