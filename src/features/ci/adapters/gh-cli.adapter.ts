import type { GithubApiPort, CheckRunInput, CheckRunRef, TriggerAutoMergeInput } from "../ports/github-api.port.js";

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
  readonly getPullRequestAuthor = async (input: { repository: string; pr: number }): Promise<string> => {
    const result = spawnGh(
      ["api", `repos/${input.repository}/pulls/${input.pr}`, "--jq", ".user.login"],
      "",
    );

    if (result.exitCode !== 0) {
      const stderrTail = result.stderr.slice(-500);
      throw new Error(
        `gh api getPullRequestAuthor failed (exit ${result.exitCode}): ${stderrTail}`,
      );
    }

    return result.stdout.trim();
  };

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

  readonly listOpenPullRequests = async (input: { repository: string }): Promise<readonly number[]> => {
    const result = spawnGh(
      ["api", `repos/${input.repository}/pulls?state=open`, "--jq", ".[].number"],
      "",
    );

    if (result.exitCode !== 0) {
      const stderrTail = result.stderr.slice(-500);
      throw new Error(
        `gh api listOpenPullRequests failed (exit ${result.exitCode}): ${stderrTail}`,
      );
    }

    return result.stdout
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .map((line) => parseInt(line, 10))
      .filter((n) => !isNaN(n));
  };

  readonly getPullRequestFiles = async (input: { repository: string; pr: number }): Promise<readonly string[]> => {
    const result = spawnGh(
      ["api", `repos/${input.repository}/pulls/${input.pr}/files`, "--jq", ".[].filename"],
      "",
    );

    if (result.exitCode !== 0) {
      const stderrTail = result.stderr.slice(-500);
      throw new Error(
        `gh api getPullRequestFiles failed (exit ${result.exitCode}): ${stderrTail}`,
      );
    }

    return result.stdout
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
  };

  readonly triggerAutoMerge = async (input: TriggerAutoMergeInput): Promise<void> => {
    const args: string[] = ["pr", "merge", String(input.pr), "--auto", "--repo", input.repository];
    if (input.mergeMethod === "squash") {
      args.push("--squash");
    } else if (input.mergeMethod === "rebase") {
      args.push("--rebase");
    } else {
      args.push("--merge");
    }

    const result = spawnGh(args, "");

    if (result.exitCode !== 0) {
      const stderrTail = result.stderr.slice(-500);
      throw new Error(
        `gh pr merge --auto failed (exit ${result.exitCode}): ${stderrTail}`,
      );
    }
  };
}
