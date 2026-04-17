import { access, chmod, copyFile, mkdir, mkdtemp, rename, rm, unlink } from "node:fs/promises";
import { delimiter, join } from "node:path";
import {
  resolveDefaultInstallDir,
  resolveInstalledBinaryName,
} from "../src/infra/usecases/install-release-binary.usecase.js";

const platform = process.platform;
const sourceBin = process.argv[2] ?? join("dist", resolveInstalledBinaryName(platform));
const installDir = process.env.MAESTRO_INSTALL_DIR ?? resolveDefaultInstallDir(platform);
const binaryName = resolveInstalledBinaryName(platform);
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
  if (platform === "win32" && (await fileExists(targetBin))) {
    const oldPath = `${targetBin}.old`;
    await unlink(oldPath).catch(() => undefined);
    await rename(targetBin, oldPath);
  }
  await rename(tempBin, targetBin);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

const resolvedOnPath = await resolveOnPath(binaryName);
const version = await readInstalledVersion(targetBin);

console.log(`[ok] Installed maestro ${version} to ${targetBin}`);
if (resolvedOnPath) {
  console.log(`[ok] PATH maestro resolves to ${resolvedOnPath}`);
} else {
  console.log("[--] maestro is not currently on PATH");
  if (platform === "win32") {
    console.log(`     Add ${installDir} to your user PATH to use maestro from any shell`);
  }
}

async function fileExists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function resolveOnPath(name: string): Promise<string | undefined> {
  const pathEnv = process.env.PATH ?? "";
  const exts = platform === "win32"
    ? (process.env.PATHEXT ?? ".EXE;.CMD;.BAT").split(";")
    : [""];
  for (const dir of pathEnv.split(delimiter).filter(Boolean)) {
    for (const ext of exts) {
      const candidate = join(dir, ext && !name.toLowerCase().endsWith(ext.toLowerCase()) ? `${name}${ext}` : name);
      if (await fileExists(candidate)) return candidate;
    }
  }
  return undefined;
}

async function readInstalledVersion(bin: string): Promise<string> {
  const proc = Bun.spawnSync([bin, "--version"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  return proc.stdout.toString().trim() || "unknown";
}
