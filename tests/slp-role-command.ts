#!/usr/bin/env bun

import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

const [receiptPath, separator, ...command] = process.argv.slice(2);
if (!receiptPath || separator !== "--" || command.length === 0) {
  process.stderr.write("usage: slp-role-command <receipt-path> -- <command> [args...]\n");
  process.exit(2);
}

const child = Bun.spawn(command, {
  cwd: process.cwd(),
  env: process.env,
  stderr: "pipe",
  stdout: "pipe",
});
const [stdout, stderr, exitCode] = await Promise.all([
  new Response(child.stdout).text(),
  new Response(child.stderr).text(),
  child.exited,
]);
await mkdir(dirname(receiptPath), { recursive: true });
await writeFile(
  receiptPath,
  `${JSON.stringify({ command, exitCode, stderr, stdout })}\n`,
);
process.stdout.write(stdout);
process.stderr.write(stderr);
process.exitCode = exitCode;
