#!/usr/bin/env bun
// Transcripts are private: this prints names and counts only, never their text.
import { createReadStream } from "node:fs";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

const root = join(import.meta.dir, "..");
const argv = process.argv.slice(2);
const option = (name: string): string | undefined => {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : undefined;
};
const days = Number(option("--days") ?? 60);
const projects = option("--projects") ?? join(homedir(), ".claude", "projects");
const since = Date.now() - days * 24 * 60 * 60 * 1000;

const invocation =
  /(?:^|[;&|\n(`]|\$\()\s*(?:[A-Z_]+=\S+\s+)*(?:command\s+)?maestro\s+([a-z][a-z-]*)(?:\s+([a-z][a-z0-9-]*))?(?:\s+([a-z][a-z0-9-]*))?/g;

const scratch = await mkdtemp(join(tmpdir(), "maestro-usage-"));

async function cli(args: string[]): Promise<string> {
  const child = Bun.spawn([process.execPath, join(root, "bin", "maestro.ts"), ...args], {
    cwd: scratch,
    env: { ...process.env, HOME: scratch, MAESTRO_SESSION_NONE: "1" },
    stdout: "pipe",
    stderr: "ignore",
  });
  const text = await new Response(child.stdout).text();
  await child.exited;
  return text;
}

function firstWords(text: string, indent: string): string[] {
  return text
    .split("\n")
    .filter((line) => line.startsWith(indent) && line[indent.length] !== " ")
    .map((line) => line.trim().split(/\s+/)[0] as string);
}

async function surface(): Promise<{ verbs: Set<string>; recipes: Set<string>; skills: Set<string> }> {
  const verbs = new Set<string>();
  const help = await cli(["help"]);
  const roots = firstWords(help.slice(0, help.indexOf("\nenvironment:")), "  ");
  for (const verb of roots) {
    const detail = await cli([verb, "--help"]);
    const subverbs = detail.includes("subverbs:") ? firstWords(detail.slice(detail.indexOf("subverbs:")), "  ") : [];
    if (subverbs.length === 0) verbs.add(verb);
    for (const subverb of subverbs) verbs.add(`${verb} ${subverb}`);
  }
  const recipes = new Set((await cli(["recipe", "list"])).split("\n").map((line) => line.split("\t")[0] as string).filter(Boolean));
  const skills = new Set(await readdir(join(root, "src", "plugins", "skills")));
  return { verbs, recipes, skills };
}

async function transcripts(directory: string): Promise<string[]> {
  const found: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) found.push(...(await transcripts(path)));
    else if (entry.name.endsWith(".jsonl")) found.push(path);
  }
  return found;
}

const known = await surface();
await rm(scratch, { recursive: true, force: true });
const counts = new Map<string, number>();
const seen = new Set<string>();
let calls = 0;
const bump = (key: string): void => {
  calls += 1;
  counts.set(key, (counts.get(key) ?? 0) + 1);
};

for (const path of await transcripts(projects)) {
  const lines = createInterface({ input: createReadStream(path), crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line.includes('"tool_use"') || !line.includes("maestro")) continue;
    let row: { timestamp?: string; message?: { content?: unknown } };
    try {
      row = JSON.parse(line);
    } catch {
      continue;
    }
    if (!row.timestamp || Date.parse(row.timestamp) < since) continue;
    const content = row.message?.content;
    if (!Array.isArray(content)) continue;
    for (const item of content as Array<{ type?: string; id?: string; name?: string; input?: Record<string, unknown> }>) {
      if (item.type !== "tool_use" || !item.id || seen.has(item.id)) continue;
      seen.add(item.id);
      const input = item.input ?? {};
      if (item.name === "Bash") {
        for (const match of String(input.command ?? "").matchAll(invocation)) {
          const [, verb, second, third] = match;
          const pair = `${verb} ${second ?? ""}`.trim();
          bump(`verb ${known.verbs.has(pair) ? pair : verb}`);
          if (pair === "recipe show" && third) bump(`recipe ${third}`);
        }
      } else if (item.name === "Skill" && String(input.skill ?? "").startsWith("maestro-")) {
        bump(`skill ${input.skill}`);
      } else if (item.name === "Read") {
        const skill = /\/skills\/(maestro-[a-z-]+)\//.exec(String(input.file_path ?? ""));
        if (skill) bump(`skill ${skill[1]}`);
      }
    }
  }
}

const report = (kind: string, names: Set<string>): void => {
  const rows = [...names].map((name) => ({ name, count: counts.get(`${kind} ${name}`) ?? 0 }));
  const roots = new Set(rows.map((row) => row.name.split(" ")[0]));
  for (const rootVerb of kind === "verb" ? roots : []) {
    const bare = counts.get(`verb ${rootVerb}`) ?? 0;
    const row = rows.find((candidate) => candidate.name === rootVerb);
    if (!row && bare > 0) rows.push({ name: `${rootVerb} (unrecognized subverb)`, count: bare });
  }
  console.log(`\n${kind}s called in the last ${days} days:`);
  for (const row of rows.filter((candidate) => candidate.count > 0).sort((left, right) => right.count - left.count)) {
    console.log(`  ${String(row.count).padStart(5)}  ${row.name}`);
  }
  console.log(`${kind}s with no call in ${days} days (consider cutting):`);
  for (const row of rows.filter((candidate) => candidate.count === 0).sort((left, right) => left.name.localeCompare(right.name))) {
    console.log(`  ${row.name}`);
  }
};

console.log(`${calls} maestro calls since ${new Date(since).toISOString().slice(0, 10)} under ${projects}`);
report("verb", known.verbs);
report("recipe", known.recipes);
report("skill", known.skills);
