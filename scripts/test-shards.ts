#!/usr/bin/env bun
import { readdir, readFile } from "node:fs/promises";
import { availableParallelism } from "node:os";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const testsDirectory = join(root, "tests");

function shardCount(argv: string[]): number {
  const flag = argv.indexOf("--shards");
  const requested = flag >= 0 ? Number(argv[flag + 1]) : Math.min(6, availableParallelism());
  if (!Number.isInteger(requested) || requested < 1) {
    throw new Error("--shards takes a positive integer");
  }
  return requested;
}

async function weightedFiles(): Promise<{ path: string; weight: number }[]> {
  const names = (await readdir(testsDirectory)).filter((name) => name.endsWith(".test.ts")).sort();
  return Promise.all(
    names.map(async (name) => {
      const body = await readFile(join(testsDirectory, name), "utf8");
      return { path: join("tests", name), weight: body.split("\n").length };
    }),
  );
}

function pack(files: { path: string; weight: number }[], count: number): string[][] {
  const shards = Array.from({ length: count }, () => ({ files: [] as string[], weight: 0 }));
  for (const file of [...files].sort((left, right) => right.weight - left.weight)) {
    const lightest = shards.reduce((best, shard) => (shard.weight < best.weight ? shard : best));
    lightest.files.push(file.path);
    lightest.weight += file.weight;
  }
  return shards.map((shard) => shard.files).filter((list) => list.length > 0);
}

async function runShard(index: number, files: string[]): Promise<{ index: number; exitCode: number; output: string; seconds: number }> {
  const started = performance.now();
  const child = Bun.spawn(["bun", "test", "--timeout", "20000", ...files], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { index, exitCode, output: stdout + stderr, seconds: (performance.now() - started) / 1000 };
}

const started = performance.now();
const shards = pack(await weightedFiles(), shardCount(process.argv.slice(2)));
const results = await Promise.all(shards.map((files, index) => runShard(index, files)));

let failed = 0;
for (const result of results) {
  const tally = result.output
    .split("\n")
    .filter((line) => /^\s*\d+ (pass|fail|skip)$|^Ran \d+ tests/.test(line))
    .map((line) => line.trim())
    .join(", ");
  console.log(`shard ${result.index} (${shards[result.index]!.length} files, ${result.seconds.toFixed(0)}s) exit=${result.exitCode}: ${tally}`);
  if (result.exitCode !== 0) {
    failed += 1;
    console.log(result.output);
  }
}
console.log(`${results.length} shards, ${failed} failed, wall ${((performance.now() - started) / 1000).toFixed(0)}s`);
process.exit(failed === 0 ? 0 : 1);
