import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import type { SlpRoleContract } from "../src/plugins/slp-runtime.ts";

export function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

// w702: a Herdr pane id is a workspace segment rendered in base 36, so it
// carries letters once the counter passes 9 (w2D:p1, w2G:p3, w2P:p5 read off a
// live daemon), followed by a small decimal pane number. The old
// /^w\d+:p\d+$/ described no id Herdr can issue and only ever passed because
// the fake minted digit-only ids.
export const herdrPaneIdShape = /^w[0-9A-Z]+:p\d+$/;

const runtimePhaseLine =
  /^\S+: (?:starting (?:claude|codex) pane in \S+|waiting for acknowledgement \(up to \d+s\)|ready in \d+s|running in \S+|resetting (?:claude|codex) context in \S+|already (?:acknowledged|running) in \S+; left alone)$/;

// Runtime phase lines (d757) are progress, not failures.
export function phaseFree(stderr: string): string {
  return stderr
    .split("\n")
    .filter((line) => line !== "" && !runtimePhaseLine.test(line))
    .join("\n");
}

export function failureEnvelope(stderr: string): { error: Record<string, unknown> } {
  const line = phaseFree(stderr).split("\n").findLast((candidate) => candidate.startsWith("{"));
  return JSON.parse(line ?? "{}") as { error: Record<string, unknown> };
}

export function watchRuntimeDirectory(projectPath: string, teamId: string, generation: number): string {
  const projectKey = createHash("sha256")
    .update(realpathSync.native(resolve(projectPath)))
    .digest("hex")
    .slice(0, 16);
  const user = typeof process.getuid === "function" ? String(process.getuid()) : "user";
  return join(tmpdir(), `maestro-slp-${user}`, projectKey, teamId, `g${generation}`);
}

export function testRoleContract(teamId: string, generation: number): SlpRoleContract {
  const instanceId = "00000000-0000-4000-8000-000000000001";
  const packDigest = "a".repeat(64);
  const briefDigest = "b".repeat(64);
  const readyChallenge = "c".repeat(32);
  const acknowledgement = [
    "SLP_ROLE_READY",
    `team=${teamId}`,
    `generation=${generation}`,
    "role=lead",
    `challenge=${readyChallenge}`,
  ].join(" ");
  return {
    acknowledgement,
    body: `slp team ${teamId} generation ${generation} instance ${instanceId}; reply ${readyChallenge}`,
    briefDigest,
    instanceId,
    packDigest,
    readyChallenge,
  };
}

export async function waitForText(path: string, needle: string, timeoutMs = 3_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await Bun.file(path).exists()) {
      const text = await Bun.file(path).text();
      if (text.includes(needle)) return;
    }
    await Bun.sleep(25);
  }
  throw new Error(`timed out waiting for ${needle} in ${path}`);
}
