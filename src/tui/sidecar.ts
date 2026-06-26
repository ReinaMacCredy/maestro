#!/usr/bin/env bun
import { readFile } from "node:fs/promises";

import { renderDashboard, renderPreviewFrame, runRenderCheck } from "@/tui/opentui/index.js";
import { isPreviewScreen, type PreviewScreen } from "@/tui/app/preview-state.js";
import { adaptRustSnapshot, type RustMissionControlSnapshot } from "@/tui/current-snapshot.js";

interface SidecarArgs {
  readonly mode: "preview" | "render-check" | "interactive";
  readonly snapshotFile: string;
  readonly cwd: string;
  readonly maestroBin?: string;
  readonly screen?: string;
  readonly feature?: string;
  readonly width?: number;
  readonly height?: number;
  readonly format?: "plain" | "ansi";
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const rustSnapshot = await readRustSnapshot(args.snapshotFile);
  const snapshot = adaptRustSnapshot(rustSnapshot);

  if (args.mode === "render-check") {
    const result = await runRenderCheck(snapshot, {
      width: args.width,
      height: args.height,
    });
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  if (args.mode === "interactive") {
    await renderDashboard({
      snapshot,
      snapshotDeps: { config: {} },
      reloadSnapshot: async () => {
        if (!args.maestroBin) return snapshot;
        try {
          return adaptRustSnapshot(await reloadRustSnapshot(args.cwd, args.maestroBin));
        } catch {
          return snapshot;
        }
      },
    });
    return;
  }

  const screens = resolvePreviewScreens(args.screen);
  const frames: string[] = [];
  for (const screen of screens) {
    frames.push(
      await renderPreviewFrame({
        snapshot,
        screen,
        featureId: args.feature,
        width: args.width,
        height: args.height,
        format: args.format,
      }),
    );
  }
  process.stdout.write(frames.join("\n\n"));
}

function parseArgs(argv: readonly string[]): SidecarArgs {
  const flags = new Map<string, string | true>();
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg?.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (next && !next.startsWith("--")) {
      flags.set(key, next);
      index += 1;
    } else {
      flags.set(key, true);
    }
  }

  const mode = readString(flags, "mode");
  if (mode !== "preview" && mode !== "render-check" && mode !== "interactive") {
    throw new Error("--mode must be preview, render-check, or interactive");
  }
  const snapshotFile = readString(flags, "snapshot-file");
  if (!snapshotFile) throw new Error("--snapshot-file is required");
  const size = parseSize(readString(flags, "size"));
  const format = readString(flags, "format");
  if (!isOutputFormat(format)) {
    throw new Error("--format must be plain or ansi");
  }

  return {
    mode,
    snapshotFile,
    cwd: readString(flags, "cwd") || process.cwd(),
    maestroBin: readString(flags, "maestro-bin") || undefined,
    screen: readString(flags, "screen") || undefined,
    feature: readString(flags, "feature") || undefined,
    width: size?.width,
    height: size?.height,
    format: format || undefined,
  };
}

function isOutputFormat(value: string): value is "plain" | "ansi" | "" {
  return value === "" || value === "plain" || value === "ansi";
}

function readString(flags: ReadonlyMap<string, string | true>, key: string): string {
  const value = flags.get(key);
  return typeof value === "string" ? value : "";
}

function parseSize(value: string): { readonly width: number; readonly height: number } | undefined {
  if (!value) return undefined;
  const match = /^(\d+)x(\d+)$/i.exec(value);
  if (!match) throw new Error(`invalid --size '${value}', expected WxH`);
  return {
    width: Number(match[1]),
    height: Number(match[2]),
  };
}

function resolvePreviewScreens(raw: string | undefined): PreviewScreen[] {
  const screen = raw || "dashboard";
  if (screen === "all") {
    return [
      "dashboard",
      "features",
      "agents",
      "dispatch",
      "events",
      "tasks",
      "timeline",
      "principles",
      "config",
      "memory",
      "graph",
      "help",
    ];
  }
  const aliases: Readonly<Record<string, PreviewScreen>> = {
    cards: "features",
    card: "features",
    activity: "events",
    proof: "principles",
    verify: "principles",
  };
  const resolved = aliases[screen] ?? screen;
  if (!isPreviewScreen(resolved)) {
    throw new Error(`unknown OpenTUI preview screen '${screen}'`);
  }
  return [resolved];
}

async function readRustSnapshot(path: string): Promise<RustMissionControlSnapshot> {
  const text = await readFile(path, "utf8");
  return JSON.parse(text) as RustMissionControlSnapshot;
}

async function reloadRustSnapshot(cwd: string, maestroBin: string): Promise<RustMissionControlSnapshot> {
  const proc = Bun.spawn([maestroBin, "mission-control", "--json"], {
    cwd,
    env: {
      ...process.env,
      MAESTRO_AUTO_UPDATE: "0",
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (code !== 0) {
    throw new Error(stderr.trim() || `maestro mission-control --json exited ${code}`);
  }
  return JSON.parse(stdout) as RustMissionControlSnapshot;
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`mission-control OpenTUI sidecar failed: ${message}`);
  process.exitCode = 1;
});
