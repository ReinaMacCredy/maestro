import { join } from "node:path";
import type { GitSignatureProbePort } from "../ports/git-signature.port.js";

export class GitSignatureAdapter implements GitSignatureProbePort {
  async showSignatureLog(input: {
    readonly repoRoot: string;
    readonly base: string;
    readonly head: string;
  }): Promise<string> {
    const proc = Bun.spawn(
      ["git", "log", "--show-signature", `${input.base}..${input.head}`],
      { cwd: input.repoRoot, stdout: "pipe", stderr: "pipe" },
    );
    const [stdout] = await Promise.all([
      new Response(proc.stdout).text(),
      proc.exited,
    ]);
    return stdout;
  }
}
