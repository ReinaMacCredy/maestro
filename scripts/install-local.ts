import { chmod, copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileExists } from "../src/shared/lib/fs.js";
import { readInstalledVersion } from "./install-local-lib";
import {
  replaceInstalledBinary,
  resolveInstallDir,
  resolveInstalledBinaryName,
} from "../src/infra/usecases/install-release-binary.usecase.js";
import {
  buildMaestroAgentMcpConfigEntry,
  configureAgentRuntime,
  defaultAgentRuntimeTargets,
  resolveMaestroBinaryInstallPath,
} from "../src/features/mcp/usecases/configure-agent-runtime.usecase.js";

const platform = process.platform;
const binaryName = resolveInstalledBinaryName(platform);
const sourceBin = process.argv[2] ?? join("dist", binaryName);
const installDir = resolveInstallDir(platform);
const targetBin = join(installDir, binaryName);

if (!(await fileExists(sourceBin))) {
  console.error(`[!!] Built binary not found at ${sourceBin}`);
  console.error("     Run: bun run build");
  process.exit(1);
}

await mkdir(installDir, { recursive: true });

const tempDir = await mkdtemp(join(installDir, ".maestro.tmp."));
const tempBin = join(tempDir, binaryName);
try {
  await copyFile(sourceBin, tempBin);
  if (platform !== "win32") {
    await chmod(tempBin, 0o755);
  }
  await replaceInstalledBinary(tempBin, targetBin, platform);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

const resolvedOnPath = Bun.which(binaryName) ?? undefined;
const version = readInstalledVersion(targetBin);

console.log(`[ok] Installed maestro ${version} to ${targetBin}`);
if (resolvedOnPath) {
  console.log(`[ok] PATH maestro resolves to ${resolvedOnPath}`);
} else {
  console.log("[--] maestro is not currently on PATH");
  if (platform === "win32") {
    console.log(`     Add ${installDir} to your user PATH to use maestro from any shell`);
  }
}

// MCP server: configure detected agent runtimes (Claude Code, Codex) to
// launch the Bun-compiled maestro binary directly. We shell out to the
// agent CLIs (`claude mcp add -s user`, `codex mcp add`) so the entry lands
// in the location each runtime actually reads — `~/.claude.json` for Claude
// Code, `~/.codex/config.toml` for Codex. The binary embeds its own runtime,
// so no separate Node bundle is required.
const binaryPath = resolveMaestroBinaryInstallPath(installDir, platform);
const configEntry = buildMaestroAgentMcpConfigEntry(binaryPath);
const runtimeResults = defaultAgentRuntimeTargets().map((target) =>
  configureAgentRuntime(target, configEntry),
);
for (const r of runtimeResults) {
  switch (r.action) {
    case "skipped-no-runtime":
      console.log(`[--] ${r.target.name}: ${r.target.cliBinary} not on PATH (skipping)`);
      break;
    case "created":
      console.log(`[ok] ${r.target.name}: registered maestro in ${r.target.configPath}`);
      break;
    case "updated":
      console.log(`[ok] ${r.target.name}: updated maestro in ${r.target.configPath}`);
      break;
    case "unchanged":
      console.log(`[ok] ${r.target.name}: maestro already current in ${r.target.configPath}`);
      break;
    case "error":
      console.log(`[!!] ${r.target.name}: failed to configure: ${r.error}`);
      break;
  }
}

// Migration hint for installs from <= 0.75.0 that wrote to paths the agents
// don't actually read. Don't auto-delete — print so the user can clean up.
const home = homedir();
const orphans = [join(home, ".claude", "mcp.json"), join(home, ".codex", "mcp.json")].filter(
  (p) => existsSync(p),
);
if (orphans.length > 0) {
  console.log("");
  console.log("[note] Found leftover MCP config files from a previous install:");
  for (const p of orphans) console.log(`         ${p}`);
  console.log("       These are no longer read by Claude Code or Codex. You can delete them:");
  console.log(`         rm ${orphans.join(" ")}`);
}

console.log("     Restart your agent runtime to pick up the new MCP server.");
