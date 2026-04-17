import { chmod, copyFile, mkdir, mkdtemp, rename, rm } from "node:fs/promises";
import { join } from "node:path";
import { fileExists, renameForInPlaceReplace } from "../src/shared/lib/fs.js";
import {
  resolveInstallDir,
  resolveInstalledBinaryName,
} from "../src/infra/usecases/install-release-binary.usecase.js";

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
  if (platform === "win32") {
    await renameForInPlaceReplace(targetBin);
  }
  await rename(tempBin, targetBin);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

const [resolvedOnPath, version] = await Promise.all([
  Promise.resolve(Bun.which(binaryName) ?? undefined),
  readInstalledVersion(targetBin),
]);

console.log(`[ok] Installed maestro ${version} to ${targetBin}`);
if (resolvedOnPath) {
  console.log(`[ok] PATH maestro resolves to ${resolvedOnPath}`);
} else {
  console.log("[--] maestro is not currently on PATH");
  if (platform === "win32") {
    console.log(`     Add ${installDir} to your user PATH to use maestro from any shell`);
  }
}

async function readInstalledVersion(bin: string): Promise<string> {
  const proc = Bun.spawnSync([bin, "--version"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  return proc.stdout.toString().trim() || "unknown";
}
