import type { GithubApiPort, CheckRunInput, CheckRunRef } from "../ports/github-api.port.js";

function buildCheckRunPayload(
  input: CheckRunInput,
): Record<string, unknown> {
  return {
    name: input.name,
    head_sha: input.headSha,
    status: "completed",
    conclusion: input.conclusion,
    output: {
      title: input.title,
      summary: input.summary,
    },
  };
}

function buildPatchPayload(
  input: Omit<CheckRunInput, "headSha" | "repository">,
): Record<string, unknown> {
  return {
    status: "completed",
    conclusion: input.conclusion,
    output: {
      title: input.title,
      summary: input.summary,
    },
  };
}

function spawnGh(
  args: string[],
  stdinPayload: string,
): { stdout: string; stderr: string; exitCode: number } {
  let proc: ReturnType<typeof Bun.spawnSync>;
  try {
    proc = Bun.spawnSync(["gh", ...args], {
      stdin: Buffer.from(stdinPayload, "utf8"),
      stdout: "pipe",
      stderr: "pipe",
      timeout: 30_000,
      env: process.env,
    });
  } catch {
    return { stdout: "", stderr: "gh command not found", exitCode: 127 };
  }

  if (proc.exitedDueToTimeout) {
    return { stdout: "", stderr: "gh command timed out after 30000ms", exitCode: 124 };
  }

  return {
    stdout: proc.stdout.toString().trim(),
    stderr: proc.stderr.toString().trim(),
    exitCode: proc.exitCode ?? 1,
  };
}

export class GhCliAdapter implements GithubApiPort {
  readonly postCheckRun = async (input: CheckRunInput): Promise<CheckRunRef> => {
    const payload = JSON.stringify(buildCheckRunPayload(input));
    const result = spawnGh(
      ["api", `repos/${input.repository}/check-runs`, "-X", "POST", "--input", "-"],
      payload,
    );

    if (result.exitCode !== 0) {
      const stderrTail = result.stderr.slice(-500);
      throw new Error(
        `gh api postCheckRun failed (exit ${result.exitCode}): ${stderrTail}`,
      );
    }

    const parsed = JSON.parse(result.stdout) as { id?: unknown };
    if (typeof parsed.id !== "number") {
      throw new Error(
        `gh api postCheckRun: unexpected response (no numeric id): ${result.stdout.slice(0, 200)}`,
      );
    }
    return { id: parsed.id };
  };

  readonly patchCheckRun = async (
    input: CheckRunInput & { readonly checkRunId: number },
  ): Promise<void> => {
    const payload = JSON.stringify(buildPatchPayload(input));
    const result = spawnGh(
      [
        "api",
        `repos/${input.repository}/check-runs/${input.checkRunId}`,
        "-X",
        "PATCH",
        "--input",
        "-",
      ],
      payload,
    );

    if (result.exitCode !== 0) {
      const stderrTail = result.stderr.slice(-500);
      throw new Error(
        `gh api patchCheckRun failed (exit ${result.exitCode}): ${stderrTail}`,
      );
    }
  };
}
