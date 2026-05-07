import { describe, it, expect, beforeAll, afterAll } from "bun:test";
import { writeFile, chmod, rm, mkdtemp } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { GhCliAdapter } from "@/features/ci/adapters/gh-cli.adapter.js";

// We exercise the adapter using a fake `gh` binary written to a temp dir.
// PATH is patched for the duration of the test to point at the fake binary.

let fakeBinDir: string;
let originalPath: string;

const FAKE_GH_SCRIPT = `#!/bin/sh
# Echo stdin as a JSON response with a fixed id so the adapter can parse it.
# The test only cares that:
#   1. The adapter invokes gh with correct positional args.
#   2. It parses the response id from stdout.
#   3. It handles non-zero exit cleanly.
case "$*" in
  *"check-runs -X POST"*|*"-X POST"*"check-runs"*)
    cat /dev/stdin > /dev/null  # consume stdin
    printf '{"id":12345}\\n'
    exit 0
    ;;
  *"check-runs/"*"-X PATCH"*|*"-X PATCH"*"check-runs/"*)
    cat /dev/stdin > /dev/null  # consume stdin
    printf '{"id":12345}\\n'
    exit 0
    ;;
  *"--fail-me"*)
    printf 'error: unauthorized\\n' >&2
    exit 1
    ;;
  *)
    printf '{"id":12345}\\n'
    exit 0
    ;;
esac
`;

beforeAll(async () => {
  fakeBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-fake-"));
  const fakeBin = join(fakeBinDir, "gh");
  await writeFile(fakeBin, FAKE_GH_SCRIPT, "utf8");
  await chmod(fakeBin, 0o755);
  originalPath = process.env.PATH ?? "";
  process.env.PATH = `${fakeBinDir}:${originalPath}`;
});

afterAll(async () => {
  process.env.PATH = originalPath;
  await rm(fakeBinDir, { recursive: true, force: true });
});

describe("GhCliAdapter — postCheckRun", () => {
  it("returns a CheckRunRef with a numeric id on success", async () => {
    const adapter = new GhCliAdapter();
    const ref = await adapter.postCheckRun({
      repository: "owner/repo",
      headSha: "deadbeef",
      name: "Maestro Verify",
      conclusion: "success",
      title: "Maestro Verdict: PASS",
      summary: "Effective risk class: medium.\n- all checks passed",
    });
    expect(ref.id).toBe(12345);
  });

  it("throws a descriptive error when gh exits non-zero", async () => {
    // Write a failing variant of the fake binary.
    const failBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-fail-"));
    const failBin = join(failBinDir, "gh");
    await writeFile(
      failBin,
      "#!/bin/sh\nprintf 'error: bad credentials\\n' >&2\nexit 1\n",
      "utf8",
    );
    await chmod(failBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${failBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      await expect(
        adapter.postCheckRun({
          repository: "owner/repo",
          headSha: "deadbeef",
          name: "Maestro Verify",
          conclusion: "failure",
          title: "Maestro Verdict: FAIL",
          summary: "Effective risk class: high.",
        }),
      ).rejects.toThrow(/postCheckRun failed.*exit 1/);
    } finally {
      process.env.PATH = savedPath;
      await rm(failBinDir, { recursive: true, force: true });
    }
  });
});

describe("GhCliAdapter — patchCheckRun", () => {
  it("resolves without error on success", async () => {
    const adapter = new GhCliAdapter();
    await expect(
      adapter.patchCheckRun({
        repository: "owner/repo",
        headSha: "deadbeef",
        name: "Maestro Verify",
        conclusion: "action_required",
        title: "Maestro Verdict: HUMAN",
        summary: "Effective risk class: high.",
        checkRunId: 12345,
      }),
    ).resolves.toBeUndefined();
  });
});

describe("GhCliAdapter — interface conformance", () => {
  it("implements GithubApiPort (has postCheckRun, patchCheckRun, triggerAutoMerge, listOpenPullRequests, getPullRequestFiles)", () => {
    const adapter = new GhCliAdapter();
    expect(typeof adapter.postCheckRun).toBe("function");
    expect(typeof adapter.patchCheckRun).toBe("function");
    expect(typeof adapter.triggerAutoMerge).toBe("function");
    expect(typeof adapter.listOpenPullRequests).toBe("function");
    expect(typeof adapter.getPullRequestFiles).toBe("function");
  });
});

describe("GhCliAdapter — listOpenPullRequests", () => {
  it("parses newline-delimited integers from gh output", async () => {
    // Create a custom fake gh that outputs PR numbers, one per line
    const customBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-list-prs-"));
    const customBin = join(customBinDir, "gh");
    await writeFile(customBin, "#!/bin/sh\nprintf '10\\n20\\n30\\n'\nexit 0\n", "utf8");
    await chmod(customBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${customBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      const result = await adapter.listOpenPullRequests({ repository: "owner/repo" });
      expect(result).toEqual([10, 20, 30]);
    } finally {
      process.env.PATH = savedPath;
      await rm(customBinDir, { recursive: true, force: true });
    }
  });

  it("returns empty array when gh outputs nothing", async () => {
    const customBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-list-empty-"));
    const customBin = join(customBinDir, "gh");
    await writeFile(customBin, "#!/bin/sh\nprintf ''\nexit 0\n", "utf8");
    await chmod(customBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${customBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      const result = await adapter.listOpenPullRequests({ repository: "owner/repo" });
      expect(result).toEqual([]);
    } finally {
      process.env.PATH = savedPath;
      await rm(customBinDir, { recursive: true, force: true });
    }
  });

  it("throws when gh exits non-zero", async () => {
    const customBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-list-fail-"));
    const customBin = join(customBinDir, "gh");
    await writeFile(customBin, "#!/bin/sh\nprintf 'error: not found\\n' >&2\nexit 1\n", "utf8");
    await chmod(customBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${customBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      await expect(adapter.listOpenPullRequests({ repository: "owner/repo" }))
        .rejects.toThrow(/listOpenPullRequests failed.*exit 1/);
    } finally {
      process.env.PATH = savedPath;
      await rm(customBinDir, { recursive: true, force: true });
    }
  });
});

describe("GhCliAdapter — getPullRequestFiles", () => {
  it("parses newline-delimited file paths from gh output", async () => {
    const customBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-pr-files-"));
    const customBin = join(customBinDir, "gh");
    await writeFile(customBin, "#!/bin/sh\nprintf 'src/foo.ts\\nsrc/bar.ts\\n'\nexit 0\n", "utf8");
    await chmod(customBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${customBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      const result = await adapter.getPullRequestFiles({ repository: "owner/repo", pr: 42 });
      expect(result).toEqual(["src/foo.ts", "src/bar.ts"]);
    } finally {
      process.env.PATH = savedPath;
      await rm(customBinDir, { recursive: true, force: true });
    }
  });

  it("returns empty array when gh outputs nothing", async () => {
    const customBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-pr-files-empty-"));
    const customBin = join(customBinDir, "gh");
    await writeFile(customBin, "#!/bin/sh\nprintf ''\nexit 0\n", "utf8");
    await chmod(customBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${customBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      const result = await adapter.getPullRequestFiles({ repository: "owner/repo", pr: 42 });
      expect(result).toEqual([]);
    } finally {
      process.env.PATH = savedPath;
      await rm(customBinDir, { recursive: true, force: true });
    }
  });

  it("throws when gh exits non-zero", async () => {
    const customBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-pr-files-fail-"));
    const customBin = join(customBinDir, "gh");
    await writeFile(customBin, "#!/bin/sh\nprintf 'error: not found\\n' >&2\nexit 1\n", "utf8");
    await chmod(customBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${customBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      await expect(adapter.getPullRequestFiles({ repository: "owner/repo", pr: 42 }))
        .rejects.toThrow(/getPullRequestFiles failed.*exit 1/);
    } finally {
      process.env.PATH = savedPath;
      await rm(customBinDir, { recursive: true, force: true });
    }
  });
});

describe("GhCliAdapter — triggerAutoMerge", () => {
  it("resolves without error when gh exits 0 (default merge method)", async () => {
    const adapter = new GhCliAdapter();
    await expect(
      adapter.triggerAutoMerge({
        repository: "owner/repo",
        pr: 42,
      }),
    ).resolves.toBeUndefined();
  });

  it("resolves without error when gh exits 0 (squash method)", async () => {
    const adapter = new GhCliAdapter();
    await expect(
      adapter.triggerAutoMerge({
        repository: "owner/repo",
        pr: 42,
        mergeMethod: "squash",
      }),
    ).resolves.toBeUndefined();
  });

  it("resolves without error when gh exits 0 (rebase method)", async () => {
    const adapter = new GhCliAdapter();
    await expect(
      adapter.triggerAutoMerge({
        repository: "owner/repo",
        pr: 42,
        mergeMethod: "rebase",
      }),
    ).resolves.toBeUndefined();
  });

  it("throws a descriptive error when gh exits non-zero", async () => {
    const failBinDir = await mkdtemp(join(tmpdir(), "maestro-gh-fail-auto-"));
    const failBin = join(failBinDir, "gh");
    await writeFile(
      failBin,
      "#!/bin/sh\nprintf 'error: pull request is not open\\n' >&2\nexit 1\n",
      "utf8",
    );
    await chmod(failBin, 0o755);
    const savedPath = process.env.PATH;
    process.env.PATH = `${failBinDir}:${originalPath}`;

    try {
      const adapter = new GhCliAdapter();
      await expect(
        adapter.triggerAutoMerge({
          repository: "owner/repo",
          pr: 99,
        }),
      ).rejects.toThrow(/gh pr merge --auto failed.*exit 1/);
    } finally {
      process.env.PATH = savedPath;
      await rm(failBinDir, { recursive: true, force: true });
    }
  });
});
