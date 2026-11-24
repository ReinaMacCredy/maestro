import { join } from "node:path";

// v0.126.0's `maestro update` imports this path after the runtime swap and calls materializeProfiles; without it the first update to this version fails.
export async function materializeProfiles(home: string, repo: string): Promise<void> {
  const child = Bun.spawn([process.execPath, join(import.meta.dir, "..", "..", "bin", "maestro.ts"), "update", "--after-swap"], {
    cwd: repo,
    env: { ...process.env, HOME: home },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stderr, exitCode] = await Promise.all([new Response(child.stderr).text(), child.exited, new Response(child.stdout).text()]);
  if (exitCode !== 0) throw new Error(`maestro update --after-swap failed (${exitCode}): ${stderr.trim()}`);
}
