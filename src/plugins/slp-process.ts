import { readFile, rm, writeFile } from "node:fs/promises";

function pidIsLive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

export async function acquireProcessLock(lockPath: string, label: string): Promise<void> {
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      await writeFile(lockPath, `${process.pid}\n`, { flag: "wx" });
      return;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      const holder = Number((await readFile(lockPath, "utf8").catch(() => "")).trim());
      if (Number.isInteger(holder) && holder > 0 && pidIsLive(holder)) {
        throw new Error(`${label} already running for this team generation (pid ${holder})`);
      }
      await rm(lockPath, { force: true });
    }
  }
  throw new Error(`${label} already running for this team generation`);
}

export async function runSlpProcess(
  args: string[],
  cwd: string,
  env: Record<string, string | undefined>,
): Promise<string> {
  const child = Bun.spawn(args, {
    cwd,
    env: { ...process.env, ...env },
    stderr: "pipe",
    stdout: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(`${args.slice(0, 3).join(" ")} failed (${exitCode}): ${stderr.trim()}`);
  }
  return stdout;
}
