import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, type CliResult, type Fixture } from "./helpers.ts";

export function data<T>(result: CliResult): T {
  if (result.exitCode !== 0) {
    throw new Error(`expected success, got exit ${result.exitCode}: ${result.stderr}`);
  }
  return (JSON.parse(result.stdout) as { data: T }).data;
}

export function failure(result: CliResult): { code: string; message: string; [key: string]: unknown } {
  if (result.exitCode === 0) throw new Error(`expected failure, got: ${result.stdout}`);
  return (JSON.parse(result.stderr) as { error: { code: string; message: string } }).error;
}

export async function writeGraph(directory: string, name: string, text: string): Promise<string> {
  await mkdir(directory, { recursive: true });
  const path = join(directory, `${name}.md`);
  await writeFile(path, text);
  return path;
}

export function repoGraphs(fixture: Fixture): string {
  return join(fixture.repo, ".maestro", "graphs");
}

export function homeGraphs(fixture: Fixture): string {
  return join(fixture.home, "maestro", "graphs");
}

// A profile source the fixture owns, so a test graph never depends on a
// shipped preset.
export async function writeProfile(fixture: Fixture, name: string): Promise<void> {
  const directory = join(fixture.home, "maestro", "profiles");
  await mkdir(directory, { recursive: true });
  await writeFile(
    join(directory, `${name}.md`),
    `---\nharness: claude\nmodel: default\ndescription: fixture ${name}\n---\nRole: ${name}.\n`,
  );
}

export interface Envelope {
  done: boolean;
  executor: string;
  failed?: { error: string; node: string };
  graph: string;
  limit?: string;
  nodes: Array<{
    inputs: Record<string, unknown>;
    instance?: string;
    kind: string;
    node: string;
    profile?: string;
    prompt: string;
    ref: string;
    round: number;
    schema?: unknown;
  }>;
  partial?: unknown;
  round: number;
  run: string;
  state: Record<string, unknown>;
  stopped?: string;
  used?: number;
  verdict?: unknown;
}

export async function graphRun(
  fixture: Fixture,
  args: string[],
  env: Record<string, string | undefined> = {},
): Promise<{ run: string; envelope: Envelope }> {
  const result = await runCli(fixture, ["graph", "run", ...args, "--json"], env);
  const envelope = data<Envelope>(result);
  return { run: envelope.run, envelope };
}

export async function graphNext(
  fixture: Fixture,
  run: string,
  env: Record<string, string | undefined> = {},
): Promise<Envelope> {
  return data<Envelope>(await runCli(fixture, ["graph", "next", run, "--json"], env));
}

export async function graphResult(
  fixture: Fixture,
  run: string,
  ref: string,
  text: string,
  extra: string[] = [],
  env: Record<string, string | undefined> = {},
): Promise<CliResult> {
  return runCli(fixture, ["graph", "result", run, ref, "--text", text, ...extra, "--json"], env);
}

export const passthroughGraph = `---
name: passthrough
description: one agent node
input:
  topic: {required: true}
nodes:
  answer: {kind: agent, profile: tester}
edges: []
---

## answer

Answer about {topic}.
`;
