import { join } from "node:path";
import {
  buildReleaseNotes,
} from "./release-notes-lib";

const root = join(import.meta.dir, "..");

function fail(message: string): never {
  console.error(`[!] ${message}`);
  process.exit(1);
}

function parseArgs(argv: readonly string[]): {
  readonly version: string;
  readonly output?: string;
  readonly changelogPath: string;
} {
  let version: string | undefined;
  let output: string | undefined;
  let changelogPath = join(root, "CHANGELOG.md");

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];

    if (arg === "--version") {
      if (!next) fail("Missing value for --version");
      version = next;
      index += 1;
      continue;
    }

    if (arg === "--output") {
      if (!next) fail("Missing value for --output");
      output = next;
      index += 1;
      continue;
    }

    if (arg === "--changelog") {
      if (!next) fail("Missing value for --changelog");
      changelogPath = next;
      index += 1;
      continue;
    }
  }

  if (!version) fail("Missing required --version");

  return { version, output, changelogPath };
}

async function runGit(
  args: readonly string[],
): Promise<{ readonly exitCode: number; readonly stdout: string; readonly stderr: string }> {
  const proc = Bun.spawn(["git", ...args], {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
  });

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  return {
    exitCode,
    stdout: stdout.trim(),
    stderr: stderr.trim(),
  };
}

async function findPreviousTag(): Promise<string | undefined> {
  for (const rev of ["HEAD^", "HEAD"]) {
    const result = await runGit(["describe", "--tags", "--abbrev=0", "--match", "v*", rev]);
    if (result.exitCode === 0 && result.stdout.length > 0) {
      return result.stdout;
    }
  }

  return undefined;
}

async function loadCommitSubjects(
  version: string,
  previousTag?: string,
): Promise<string[]> {
  const range = previousTag ? `${previousTag}..HEAD` : "HEAD";
  const result = await runGit(["log", "--format=%s", range]);
  if (result.exitCode !== 0) {
    fail(`git log failed for range ${range}: ${result.stderr}`);
  }

  return result.stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .filter((line) => line !== `chore(release): v${version}`);
}

const { version, output, changelogPath } = parseArgs(process.argv.slice(2));
const previousTag = await findPreviousTag();
const changelog = await Bun.file(changelogPath).exists()
  ? await Bun.file(changelogPath).text()
  : "";
const commitSubjects = await loadCommitSubjects(version, previousTag);
const notes = buildReleaseNotes({
  version,
  changelog,
  commitSubjects,
  previousTag,
});

if (output) {
  await Bun.write(output, notes);
} else {
  process.stdout.write(notes);
}
