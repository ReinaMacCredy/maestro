import { expect, test } from "bun:test";
import fc from "fast-check";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture, type Fixture } from "./helpers.ts";

type Actor = "a" | "b";

interface Step {
  actor: Actor;
  argv: string[];
}

interface WorkRow {
  id: string;
  state: string;
  parentId: string | null;
  heldBy: string | null;
}

const sessions: Record<Actor, Record<string, string>> = {
  a: { MAESTRO_SESSION_ID: "property-a", MAESTRO_SESSION_PID: "1" },
  b: { MAESTRO_SESSION_ID: "property-b", MAESTRO_SESSION_PID: String(process.pid) },
};

const corpusDirectory = join(import.meta.dir, "differential", "corpus");
const corpusSeed = 20261125;
const corpusFiles = 4;

const draw = fc.record({
  kind: fc.constantFrom("add", "add", "child", "child", "start", "start", "done", "done", "note", "release", "cancel", "decision"),
  actor: fc.constantFrom<Actor>("a", "b"),
  pick: fc.nat(1000),
});

// Every add succeeds, so the n-th add is w<n> and a sequence is fixed text
// that the differential corpus can replay.
const sequence = fc.array(draw, { minLength: 8, maxLength: 24 }).map((draws): Step[] => {
  const steps: Step[] = [];
  let works = 0;
  let token = 0;
  for (const { kind, actor, pick } of draws) {
    const target = works === 0 ? null : `w${(pick % works) + 1}`;
    if (kind === "add" || target === null) {
      token += 1;
      works += 1;
      steps.push({ actor, argv: ["work", "add", `survey zq${token}x`, "--kind", "research"] });
    } else if (kind === "child") {
      token += 1;
      works += 1;
      steps.push({ actor, argv: ["work", "add", `build zq${token}x`, "--kind", "task", "--parent", target] });
    } else if (kind === "start") {
      steps.push({ actor, argv: ["work", "start", target] });
    } else if (kind === "done") {
      steps.push({ actor, argv: ["work", "done", target, "--evidence", "bun test: 3 pass"] });
    } else if (kind === "note") {
      token += 1;
      steps.push({ actor, argv: ["work", "note", target, `observed zq${token}x`] });
    } else if (kind === "release") {
      steps.push({ actor, argv: ["work", "release", target] });
    } else if (kind === "cancel") {
      steps.push({ actor, argv: ["work", "cancel", target, "--reason", "dropped"] });
    } else {
      token += 1;
      steps.push({ actor, argv: ["decision", "draft", `prefer zq${token}x`, "--rationale", "measured"] });
    }
  }
  return steps;
});

async function works(fixture: Fixture): Promise<Map<string, WorkRow>> {
  const listed = await runCli(fixture, ["work", "list", "--json"], sessions.a);
  const rows = (JSON.parse(listed.stdout) as { data: { works: WorkRow[] } }).data.works;
  return new Map(rows.map((row) => [row.id, row]));
}

function render(steps: Step[]): string {
  const quote = (word: string): string => (/[\s"]/.test(word) ? `"${word}"` : word);
  return steps.map((step) => `@${step.actor} ${step.argv.map(quote).join(" ")}`).join("\n") + "\n";
}

test("work and decision invariants hold over random command sequences from two sessions", async () => {
  await fc.assert(
    fc.asyncProperty(sequence, async (steps) => {
      await withFixture(async (fixture) => {
        const finished = new Set<string>();
        const written = new Map<string, string>();
        const decisions = new Set<string>();
        let before = await works(fixture);

        for (const step of steps) {
          const [verb, action, target] = step.argv;
          const result = await runCli(fixture, step.argv, sessions[step.actor]);
          const after = await works(fixture);
          const session = sessions[step.actor].MAESTRO_SESSION_ID;

          if (verb === "work" && action === "add" && result.exitCode === 0) {
            const fresh = [...after.keys()].filter((id) => !before.has(id));
            expect(fresh).toHaveLength(1);
            written.set(/zq\d+x/.exec(step.argv[2] as string)![0], fresh[0] as string);
          }
          if (verb === "decision" && result.exitCode === 0) {
            const id = /^(d\d+) /.exec(result.stdout)![1] as string;
            expect(decisions.has(id)).toBe(false);
            decisions.add(id);
            written.set(/zq\d+x/.exec(step.argv[2] as string)![0], id);
          }
          if (verb === "work" && action === "note" && result.exitCode === 0) {
            written.set(/zq\d+x/.exec(step.argv[3] as string)![0], target as string);
          }
          if (verb === "work" && (action === "start" || action === "done") && result.exitCode === 0) {
            const holder = before.get(target as string)?.heldBy ?? null;
            expect(holder === null || holder === session).toBe(true);
          }
          if (verb === "work" && action === "done" && result.exitCode === 0) {
            const open = [...before.values()].filter(
              (row) => row.parentId === target && (row.state === "open" || row.state === "active"),
            );
            expect(open).toEqual([]);
          }
          for (const row of after.values()) {
            if (row.state === "done") finished.add(row.id);
            if (finished.has(row.id)) expect(row.state).toBe("done");
          }
          before = after;
        }

        for (const [token, id] of written) {
          const found = await runCli(fixture, ["search", token, "--local"], sessions.a);
          expect(found.stdout).toContain(id);
        }
      });
    }),
    { numRuns: 12, seed: corpusSeed, endOnFailure: true },
  );
}, 300_000);

test("the random differential corpus is what the generator emits", async () => {
  const samples = fc.sample(sequence, { seed: corpusSeed, numRuns: corpusFiles });
  for (const [index, steps] of samples.entries()) {
    const path = join(corpusDirectory, `random-${index + 1}.seq`);
    if (process.env["MAESTRO_CORPUS_UPDATE"] === "1") await writeFile(path, render(steps));
    expect(await readFile(path, "utf8")).toBe(render(steps));
  }
});
