#!/usr/bin/env bun
/**
 * TUI development watch loop.
 * Watches src/tui/** for changes and re-renders the preview.
 *
 * Usage:
 *   bun scripts/tui-dev.ts                        # default: dashboard, 120x40
 *   bun scripts/tui-dev.ts --screen features       # specific screen
 *   bun scripts/tui-dev.ts --screen all             # all screens
 *   bun scripts/tui-dev.ts --size 200x60            # custom size
 *   bun scripts/tui-dev.ts --check                  # render-check mode (JSON)
 *   bun scripts/tui-dev.ts --feature <id>           # specific feature
 *   bun scripts/tui-dev.ts --compiled               # use target/debug/maestro instead of cargo run
 */
import { watch } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..");
const WATCH_DIR = join(ROOT, "src", "tui");

function parseArgs() {
  const args = process.argv.slice(2);
  const flags: Record<string, string | boolean> = {};
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    if (arg.startsWith("--")) {
      const key = arg.slice(2);
      const next = args[i + 1];
      if (next && !next.startsWith("--")) {
        flags[key] = next;
        i++;
      } else {
        flags[key] = true;
      }
    }
  }
  return flags;
}

function buildCommand(flags: Record<string, string | boolean>): string[] {
  const compiled = flags.compiled === true;
  const base = compiled
    ? [join(ROOT, "target", "debug", "maestro")]
    : ["cargo", "run", "--quiet", "--"];

  const cmd = [...base, "mission-control", "--renderer", "opentui"];

  if (flags.check === true) {
    cmd.push("--render-check");
  } else {
    const screen = typeof flags.screen === "string" ? flags.screen : "dashboard";
    cmd.push("--preview", screen);
  }

  if (typeof flags.size === "string") {
    cmd.push("--size", flags.size);
  }

  cmd.push("--format", "plain");

  const feature = typeof flags.feature === "string"
    ? flags.feature
    : typeof flags.mission === "string"
      ? flags.mission
      : undefined;
  if (feature !== undefined) {
    cmd.push("--feature", feature);
  }

  return cmd;
}

async function runPreview(cmd: string[]): Promise<void> {
  const proc = Bun.spawn(cmd, {
    stdout: "pipe",
    stderr: "pipe",
    cwd: ROOT,
  });

  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);

  const code = await proc.exited;

  // Clear screen for readability
  process.stdout.write("\x1b[2J\x1b[H");

  const now = new Date().toLocaleTimeString();
  console.log(`[${now}] ${cmd.join(" ")}`);
  console.log("---");

  if (code !== 0) {
    console.log(`[!] exit ${code}`);
    if (stderr.trim()) console.log(stderr.trim());
  } else {
    console.log(stdout);
  }
}

async function main() {
  const flags = parseArgs();
  const cmd = buildCommand(flags);

  console.log(`[tui-dev] watching ${WATCH_DIR}`);
  console.log(`[tui-dev] command: ${cmd.join(" ")}`);
  console.log(`[tui-dev] press Ctrl+C to stop\n`);

  // Initial render
  await runPreview(cmd);

  // Debounce: collapse rapid saves into one re-render
  let timer: ReturnType<typeof setTimeout> | undefined;
  const DEBOUNCE_MS = 300;

  watch(WATCH_DIR, { recursive: true }, (_event, filename) => {
    if (!filename || !filename.endsWith(".ts")) return;
    if (timer) clearTimeout(timer);
    timer = setTimeout(async () => {
      console.log(`\n[tui-dev] changed: ${filename}`);
      await runPreview(cmd);
    }, DEBOUNCE_MS);
  });
}

main();
