// Interface surfaces the cleanup must not move: the help text of every verb and
// subverb, the store schema, the shipped resource bytes, the SLP.md markers the
// code greps for, and every error code the CLI can throw. Each surface renders
// to one deterministic string; tests/interface-snapshot.test.ts byte-compares it
// against tests/snapshots/.
//
// Nothing here records a file path alongside what it found. A cleanup that moves
// a declaration between files is exactly what these phases do, and a snapshot
// that failed on it would be noise; a declaration that disappears or changes
// name is the thing worth catching.

import { Database } from "bun:sqlite";
import { readdir, readFile, rm } from "node:fs/promises";
import { join, relative } from "node:path";
import { enableSlp, runCli, withFixture, type Fixture } from "./helpers.ts";

const repoRoot = join(import.meta.dir, "..");
export const snapshotDirectory = join(import.meta.dir, "snapshots");

async function sourceFiles(): Promise<string[]> {
  const glob = new Bun.Glob("src/**/*.ts");
  const found: string[] = [];
  for await (const file of glob.scan({ cwd: repoRoot })) found.push(file);
  return found.sort();
}

// ------------------------------------------------------------------- help

// The verb and subverb lists come from the CLI's own help rather than from the
// plugin registry: the snapshot is of the interface a user meets, so it has to
// be read the way a user reads it.
function listedNames(text: string, section: string): string[] {
  const lines = text.split("\n");
  const start = lines.findIndex((line) => line.trim() === section);
  if (start === -1) return [];
  const names: string[] = [];
  for (const line of lines.slice(start + 1)) {
    if (line.trim() !== "" && /^\S/.test(line)) break;
    const match = /^ {2}([a-z][a-z0-9-]*) {2,}\S/.exec(line);
    if (match) names.push(match[1]!);
  }
  return names;
}

export async function helpSurface(fixture: Fixture): Promise<string> {
  const page = async (path: string[]): Promise<string> => {
    const result = await runCli(fixture, [...path, "--help"]);
    return `${result.stdout}${result.stderr}`.trimEnd();
  };

  const top = await runCli(fixture, ["help"]);
  const verbs = listedNames(`${top.stdout}${top.stderr}`, "verbs:");
  if (verbs.length === 0) throw new Error("maestro help listed no verbs");

  const sections: string[] = [`### maestro help`, (await page(["help"])).trimEnd(), ""];
  for (const verb of verbs) {
    const rendered = await page([verb]);
    sections.push(`### maestro ${verb} --help`, rendered, "");
    for (const subverb of listedNames(rendered, "subverbs:")) {
      sections.push(`### maestro ${verb} ${subverb} --help`, await page([verb, subverb]), "");
    }
  }
  return `${sections.join("\n").trimEnd()}\n`;
}

// help.txt is read with the slp plugin off, its default (d879); this keeps every help page the plugin adds or changes.
export async function slpHelpSurface(fixture: Fixture): Promise<string> {
  const sections = (text: string) => text.trimEnd().split(/\n(?=### )/);
  const off = new Set(sections(await helpSurface(fixture)));
  await enableSlp(fixture);
  try {
    return `${sections(await helpSurface(fixture)).filter((section) => !off.has(section)).join("\n")}\n`;
  } finally {
    await rm(join(fixture.home, ".maestro", "config"));
  }
}

// ----------------------------------------------------------------- schema

function collapse(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

// Only the tables a plain CLI run creates can be dumped from a real store: the
// slp_* and team_* tables are migrated on the first team operation, which needs
// a Herdr pane. declaredSchemaObjects covers those by name; VERIFY.md carries
// the hand-review that covers their columns.
export async function schemaSurface(fixture: Fixture): Promise<string> {
  await runCli(fixture, ["work", "add", "snapshot probe"]);
  const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
  try {
    const rows = database
      .query<{ name: string; sql: string | null; type: string }, []>(
        "select type, name, sql from sqlite_master order by type, name",
      )
      .all();
    return `${rows
      .map((row) => `${row.type} ${row.name}\n  ${row.sql === null ? "(implicit)" : collapse(row.sql)}`)
      .join("\n")}\n`;
  } finally {
    database.close();
  }
}

// Every schema object the source declares, including the ones a test cannot
// materialise. Kind and name only, so moving a migration between files is free.
export async function declaredSchemaObjectsSurface(): Promise<string> {
  const pattern = /CREATE\s+(TABLE|UNIQUE INDEX|INDEX|TRIGGER|VIEW)(?:\s+IF NOT EXISTS)?\s+([A-Za-z_][A-Za-z0-9_]*)/g;
  const objects = new Set<string>();
  for (const file of await sourceFiles()) {
    const text = await readFile(join(repoRoot, file), "utf8");
    for (const match of text.matchAll(pattern)) {
      objects.add(`${match[1]!.toLowerCase()} ${match[2]!}`);
    }
  }
  return `${[...objects].sort().join("\n")}\n`;
}

// --------------------------------------------------------------- resources

export async function resourcesSurface(): Promise<string> {
  const root = join(repoRoot, "src", "plugins", "resources");
  const walk = async (directory: string): Promise<string[]> => {
    const entries = await readdir(directory, { withFileTypes: true });
    const found: string[] = [];
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) found.push(...(await walk(path)));
      else found.push(path);
    }
    return found;
  };
  const files = (await walk(root)).sort();
  const lines: string[] = [];
  for (const file of files) {
    const bytes = await readFile(file);
    const digest = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
    lines.push(`${digest}  ${relative(repoRoot, file)}`);
  }
  return `${lines.join("\n")}\n`;
}

export async function slpMarkersSurface(): Promise<string> {
  const text = await readFile(join(repoRoot, "src", "plugins", "resources", "SLP.md"), "utf8");
  const markers = text.split("\n").filter((line) => /^<!--\s*slp:/.test(line.trim()));
  return `${markers.map((line) => line.trim()).join("\n")}\n`;
}

// ------------------------------------------------------------ error codes

export async function errorCodesSurface(): Promise<string> {
  const pattern = /new CliError\(\s*"([A-Z][A-Z0-9_]*)"/g;
  const codes = new Set<string>();
  for (const file of await sourceFiles()) {
    const text = await readFile(join(repoRoot, file), "utf8");
    for (const match of text.matchAll(pattern)) codes.add(match[1]!);
  }
  return `${[...codes].sort().join("\n")}\n`;
}

// --------------------------------------------------------------- exports

// The shape tool counts exported names, and a count cannot tell a rename from a
// no-op: removing X and adding Y leaves 315 at 315 (w733 finding 4). This
// surface is the sorted set of the names themselves, so a rename lands as one
// removal and one addition. Names only, no file paths, for the reason at the top
// of this file.
//
// Value exports come from Bun's transpiler, which is a real parse. It erases
// type-only exports, so those are read off the two forms this repo actually
// uses. The set holds 314 names against the shape tool's 315 because
// `SlpRuntimeError` is declared in herdr-client.ts and re-exported by
// slp-runtime.ts, and the tool keys by file-and-name while this keys by name.
export async function exportedNamesSurface(): Promise<string> {
  const transpiler = new Bun.Transpiler({ loader: "ts" });
  const names = new Set<string>();
  for (const file of await sourceFiles()) {
    const text = await readFile(join(repoRoot, file), "utf8");
    for (const name of transpiler.scan(text).exports) names.add(name);
    for (const match of text.matchAll(/^export\s+(?:declare\s+)?(?:interface|type)\s+([A-Za-z_$][\w$]*)/gm)) {
      names.add(match[1]!);
    }
    for (const match of text.matchAll(/^export\s+type\s*\{([^}]*)\}/gm)) {
      for (const part of (match[1] ?? "").split(",")) {
        const name = part.trim().split(/\s+as\s+/).pop();
        if (name) names.add(name);
      }
    }
  }
  return `${[...names].sort().join("\n")}\n`;
}

// ------------------------------------------------------------------ suite

export interface Surface {
  name: string;
  render: (fixture: Fixture) => Promise<string>;
}

export const surfaces: Surface[] = [
  { name: "help.txt", render: helpSurface },
  { name: "help-slp.txt", render: slpHelpSurface },
  { name: "schema.txt", render: schemaSurface },
  { name: "schema-objects.txt", render: () => declaredSchemaObjectsSurface() },
  { name: "resources.txt", render: () => resourcesSurface() },
  { name: "slp-markers.txt", render: () => slpMarkersSurface() },
  { name: "error-codes.txt", render: () => errorCodesSurface() },
  { name: "exports.txt", render: () => exportedNamesSurface() },
];

export async function renderSurfaces(): Promise<Map<string, string>> {
  return withFixture(async (fixture) => {
    const rendered = new Map<string, string>();
    for (const surface of surfaces) rendered.set(surface.name, await surface.render(fixture));
    return rendered;
  });
}
