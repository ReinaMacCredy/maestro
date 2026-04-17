import { chmod, copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { fileExists } from "../src/shared/lib/fs.js";
import { readInstalledVersion } from "./install-local-lib";
import {
  replaceInstalledBinary,
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
