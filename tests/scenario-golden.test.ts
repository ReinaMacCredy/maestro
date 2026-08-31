import { expect, test } from "bun:test";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

const directory = join(import.meta.dir, "scenarios");
const recording = process.env["MAESTRO_GOLDEN_UPDATE"] === "1";

// A scenario line is a maestro command, optionally prefixed with @<session> to run
// it as another lane; quotes group an argument the way a shell would.
function tokenize(line: string): string[] {
  const tokens = line.match(/"[^"]*"|\S+/g) ?? [];
  return tokens.map((token) => (token.startsWith('"') ? token.slice(1, -1) : token));
}

function quoted(token: string): string {
  return /\s/.test(token) ? `"${token}"` : token;
}

function normalize(text: string, fixture: Fixture): string {
  return text
    .replaceAll(fixture.root, "<fixture>")
    .replace(/\d{4}-\d{2}-\d{2}T[\d:.]+Z/g, "<time>")
    .replace(/\d{4}-\d{2}-\d{2}/g, "<date>")
    .replace(/\b\d+ minutes? ago\b/g, "<age> ago")
    .replace(new RegExp(String(process.pid), "g"), "<pid>")
    .trimEnd();
}

async function replay(script: string): Promise<string> {
  return withFixture(async (fixture) => {
    const transcript: string[] = [];
    for (const raw of script.split("\n")) {
      const line = raw.trim();
      if (line === "") continue;
      if (line.startsWith("#")) {
        transcript.push(line);
        continue;
      }
      const spoken = line.startsWith("@") ? line.slice(1) : `test-session ${line}`;
      const [session, ...tokens] = tokenize(spoken);
      const result = await runCli(fixture, tokens, {
        MAESTRO_SESSION_ID: session,
        MAESTRO_SESSION_PID: String(process.pid),
      });
      transcript.push(`$ maestro ${tokens.map(quoted).join(" ")}`);
      if (result.exitCode !== 0) transcript.push(`! exit ${result.exitCode}`);
      for (const stream of [result.stdout, result.stderr]) {
        const body = normalize(stream, fixture);
        if (body !== "") transcript.push(body);
      }
      transcript.push("");
    }
    return `${transcript.join("\n").trimEnd()}\n`;
  });
}

test("557 every SLP scenario replays to its golden transcript (d43)", async () => {
  const scripts = (await readdir(directory)).filter((name) => name.endsWith(".script")).sort();
  expect(scripts.length).toBeGreaterThan(0);

  for (const name of scripts) {
    const script = await readFile(join(directory, name), "utf8");
    const golden = join(directory, name.replace(/\.script$/, ".golden"));
    const replayed = await replay(script);
    if (recording) {
      await writeFile(golden, replayed);
      continue;
    }
    const expected = await readFile(golden, "utf8");
    expect(`${name}\n${replayed}`).toBe(`${name}\n${expected}`);
  }
}, 120_000);

test("558 the replay gate is named where the improver reads it (d43)", async () => {
  const scripts = (await readdir(directory)).filter((name) => name.endsWith(".script"));
  const goldens = new Set((await readdir(directory)).filter((name) => name.endsWith(".golden")));
  for (const name of scripts) {
    expect(goldens.has(name.replace(/\.script$/, ".golden"))).toBe(true);
  }
  for (const name of goldens) {
    const golden = await readFile(join(directory, name), "utf8");
    expect(golden).not.toContain("maestro-stage1-");
  }

  const root = join(import.meta.dir, "..");
  const skill = await readFile(
    join(root, "src", "plugins", "skills", "maestro-improve", "SKILL.md"),
    "utf8",
  );
  expect(skill).toContain("tests/scenario-golden.test.ts");
  expect(skill).toContain("MAESTRO_GOLDEN_UPDATE=1");
  expect(skill).toContain("tests/scenarios/<name>.script");
  expect(skill).not.toContain("<case>");
});
