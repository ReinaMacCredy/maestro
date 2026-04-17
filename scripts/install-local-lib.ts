interface SpawnSyncLikeResult {
  readonly exitCode: number | null;
  readonly stdout: Uint8Array;
  readonly stderr: Uint8Array;
}

type SpawnSyncLike = (
  argv: string[],
  options: { stdout: "pipe"; stderr: "pipe" },
) => SpawnSyncLikeResult;

export function readInstalledVersion(
  bin: string,
  spawnSyncImpl: SpawnSyncLike = Bun.spawnSync,
): string {
  const proc = spawnSyncImpl([bin, "--version"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  if (proc.exitCode !== 0) {
    const stderr = proc.stderr.toString().trim();
    throw new Error(
      `Installed binary failed version verification (${proc.exitCode}): ${stderr || "no stderr output"}`,
    );
  }

  const version = proc.stdout.toString().trim();
  if (!version) {
    throw new Error("Installed binary did not print a version");
  }
  return version;
}
