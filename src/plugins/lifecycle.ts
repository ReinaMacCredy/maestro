import { Database } from "bun:sqlite";
import { existsSync, realpathSync } from "node:fs";
import { cp, mkdir, readFile, rename, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { Cli, CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { resolveStoreLocation } from "../kernel/store.ts";
import {
  readGitHeadCommit,
  stampRuntime,
  uninstallRepo,
} from "./install.ts";
import { readInstallStamp } from "./install-stamp.ts";
import { formatSkillSync, materializeSkills } from "./skills.ts";
import { readSourceRecord } from "./source-record.ts";

interface CommandResult {
  exitCode: number;
  stderr: string;
  stdout: string;
}

interface DoctorIssue {
  component: string;
  fix: string;
  message: string;
}

interface PackageJson {
  version: string;
}

const runtimeEntries = ["package.json", "tsconfig.json", "bin", "src"];

function homeDirectory(): string {
  return process.env.HOME ?? process.cwd();
}

function runtimeRoot(home: string): string {
  return join(home, ".maestro", "runtime");
}

async function command(cwd: string, args: string[]): Promise<CommandResult> {
  const child = Bun.spawn(args, {
    cwd,
    env: { ...process.env, LC_ALL: "C" },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

async function requireSource(home: string): Promise<string> {
  const record = await readSourceRecord(home);
  if (record.status !== "valid") {
    throw new CliError(
      record.status === "missing" ? "SOURCE_RECORD_MISSING" : "SOURCE_RECORD_INVALID",
      "Maestro source checkout record is missing or invalid; run maestro install from the source checkout",
      { fix: "run maestro install from the Maestro source checkout" },
    );
  }
  if (!existsSync(record.record.path)) {
    throw new CliError(
      "SOURCE_CHECKOUT_MISSING",
      `recorded Maestro source checkout is missing: ${record.record.path}; run maestro install from the source checkout`,
      { fix: "run maestro install from the Maestro source checkout", path: record.record.path },
    );
  }
  return record.record.path;
}

async function stageRuntime(source: string, runtime: string): Promise<string> {
  const staged = join(dirname(runtime), `.runtime-stage-${process.pid}-${Date.now()}`);
  await rm(staged, { recursive: true, force: true });
  await mkdir(staged, { recursive: true });
  try {
    for (const entry of runtimeEntries) {
      await cp(join(source, entry), join(staged, entry), { recursive: true });
    }
    await stampRuntime(source, staged);
    return staged;
  } catch (error) {
    await rm(staged, { recursive: true, force: true });
    throw error;
  }
}

async function swapRuntime(staged: string, runtime: string): Promise<void> {
  const backup = join(dirname(runtime), `.runtime-backup-${process.pid}-${Date.now()}`);
  const hadRuntime = existsSync(runtime);
  if (hadRuntime) await rename(runtime, backup);
  try {
    await rename(staged, runtime);
    if (hadRuntime) await rm(backup, { recursive: true, force: true });
  } catch (error) {
    if (existsSync(runtime)) await rm(runtime, { recursive: true, force: true });
    if (hadRuntime && existsSync(backup)) await rename(backup, runtime);
    throw error;
  }
}

async function update(): Promise<CliResult> {
  const home = homeDirectory();
  const source = await requireSource(home);
  const runtime = runtimeRoot(home);
  const oldCommit = await readGitHeadCommit(source);
  if (!oldCommit) {
    throw new CliError(
      "SOURCE_GIT_INVALID",
      `recorded Maestro source is not a readable git checkout: ${source}; run maestro install from a valid source checkout`,
      { fix: "run maestro install from a valid Maestro source checkout" },
    );
  }
  // Untracked files are ignored: install itself leaves untracked wiring
  // (.claude/, .codex/) in a wired source checkout, and ff-only merges
  // cannot lose them.
  const dirty = await command(source, ["git", "status", "--porcelain", "--untracked-files=no"]);
  if (dirty.exitCode !== 0 || dirty.stdout) {
    throw new CliError(
      "UPDATE_SOURCE_DIRTY",
      "Maestro source checkout has local changes; commit or stash them, then run maestro update",
      { fix: "commit or stash source changes, then run maestro update" },
    );
  }
  const branch = await command(source, ["git", "symbolic-ref", "--quiet", "--short", "HEAD"]);
  if (branch.exitCode !== 0 || !branch.stdout) {
    throw new CliError(
      "UPDATE_DETACHED_HEAD",
      "Maestro source checkout is detached; check out its update branch, then run maestro update",
      { fix: "check out the Maestro source branch, then run maestro update" },
    );
  }
  const upstream = await command(source, [
    "git",
    "rev-parse",
    "--abbrev-ref",
    "--symbolic-full-name",
    "@{upstream}",
  ]);
  if (upstream.exitCode !== 0 || !upstream.stdout) {
    throw new CliError(
      "UPDATE_REMOTE_MISSING",
      "Maestro source branch has no upstream; configure its git remote, then run maestro update",
      { fix: "configure an upstream git remote, then run maestro update" },
    );
  }
  const fetched = await command(source, ["git", "fetch", "--no-tags"]);
  if (fetched.exitCode !== 0) {
    throw new CliError(
      "UPDATE_FETCH_FAILED",
      `cannot fetch the Maestro source remote: ${fetched.stderr || "git fetch failed"}; fix remote connectivity, then run maestro update`,
      { fix: "fix the source remote or network, then run maestro update" },
    );
  }
  const ancestor = await command(source, [
    "git",
    "merge-base",
    "--is-ancestor",
    oldCommit,
    upstream.stdout,
  ]);
  let aheadOnly = false;
  if (ancestor.exitCode !== 0) {
    // A source strictly ahead of its upstream has nothing to pull; still
    // resync so the runtime converges on the current source commit.
    const behind = await command(source, [
      "git",
      "merge-base",
      "--is-ancestor",
      upstream.stdout,
      oldCommit,
    ]);
    if (behind.exitCode !== 0) {
      throw new CliError(
        "UPDATE_DIVERGED",
        "Maestro source has diverged from its upstream; rebase or reset it, then run maestro update",
        { fix: "rebase or reset the Maestro source branch, then run maestro update" },
      );
    }
    aheadOnly = true;
  }
  if (!aheadOnly) {
    const merged = await command(source, ["git", "merge", "--ff-only", upstream.stdout]);
    if (merged.exitCode !== 0) {
      throw new CliError(
        "UPDATE_MERGE_FAILED",
        `cannot fast-forward the Maestro source: ${merged.stderr || "git merge --ff-only failed"}; fix the checkout, then run maestro update`,
        { fix: "fix the source checkout, then run maestro update" },
      );
    }
  }
  let staged: string | null = null;
  try {
    staged = await stageRuntime(source, runtime);
    await swapRuntime(staged, runtime);
    staged = null;
  } catch (error) {
    await command(source, ["git", "reset", "--hard", oldCommit]);
    if (staged && existsSync(staged)) await rm(staged, { recursive: true, force: true });
    throw new CliError(
      "UPDATE_RESYNC_FAILED",
      `source fast-forward was rolled back because runtime resync failed: ${error instanceof Error ? error.message : String(error)}; fix the runtime path, then run maestro update`,
      { fix: "fix the Maestro runtime path, then run maestro update" },
    );
  }
  const newCommit = await readGitHeadCommit(source);
  const packageJson = JSON.parse(
    await readFile(join(source, "package.json"), "utf8"),
  ) as PackageJson;
  const skillSync = newCommit ? await materializeSkills(home, newCommit) : null;
  const skillText = skillSync ? formatSkillSync(skillSync) : "";
  return {
    data: { aheadOnly, oldCommit, newCommit, version: packageJson.version },
    text: (aheadOnly
      ? `${oldCommit} up to date (source ahead of upstream; nothing to pull)\nmaestro ${packageJson.version}`
      : `${oldCommit} -> ${newCommit}\nmaestro ${packageJson.version}`) +
      (skillText ? `\n${skillText}` : ""),
  };
}

export async function driftAdvisory(home: string, runningRoot: string): Promise<string> {
  if (process.env.MAESTRO_AUTO_UPDATE === "0") return "";
  const [stampRead, recordRead] = await Promise.all([
    readInstallStamp(runningRoot),
    readSourceRecord(home),
  ]);
  if (stampRead.status !== "valid" || recordRead.status !== "valid") return "";
  const sourceCommit = await readGitHeadCommit(recordRead.record.path);
  if (!sourceCommit || sourceCommit === stampRead.stamp.commit) return "";
  return `[update] runtime ${stampRead.stamp.commit.slice(0, 8)} differs from source ${sourceCommit.slice(0, 8)}; run maestro update`;
}

async function readJsonObject(path: string): Promise<Record<string, unknown> | null> {
  try {
    return JSON.parse(await readFile(path, "utf8")) as Record<string, unknown>;
  } catch {
    return null;
  }
}

// Codex skips repo-local hooks until their exact definition is trusted, and it
// says so only inside its own UI, so a wired repo can stay silent for months.
async function codexTrustCheck(repo: string): Promise<string> {
  const hooks = join(repo, ".codex", "hooks.json");
  if (!existsSync(hooks)) return "codex hooks: absent";
  const config = join(process.env.HOME ?? repo, ".codex", "config.toml");
  const text = existsSync(config) ? await readFile(config, "utf8") : "";
  // macOS hands out both /var/... and /private/var/... for the same repo, and
  // Codex records whichever form its own cwd had.
  const paths = new Set<string>();
  for (const candidate of [hooks, realpathSync(hooks)]) {
    paths.add(candidate);
    paths.add(
      candidate.startsWith("/private/") ? candidate.slice("/private".length) : `/private${candidate}`,
    );
  }
  return [...paths].some((path) => text.includes(`"${path}:`))
    ? "codex hooks: trusted"
    : "codex hooks: not trusted (Codex skips them; run /hooks in Codex once to trust)";
}

async function doctor(): Promise<CliResult> {
  const home = homeDirectory();
  const repo = resolve(process.cwd());
  const runtime = runtimeRoot(home);
  const checks: string[] = [];
  const issues: DoctorIssue[] = [];
  const installFix = "run maestro install from the Maestro source checkout";

  const shim = join(home, ".local", "bin", "maestro");
  if (!existsSync(shim)) {
    issues.push({ component: "shim", fix: installFix, message: `missing shim: ${shim}` });
  } else {
    const shimText = await readFile(shim, "utf8");
    const target = pathToFileURL(join(runtime, "bin", "maestro.ts")).href;
    if (!shimText.includes(target)) {
      issues.push({ component: "shim", fix: installFix, message: "shim target is stale" });
    } else {
      checks.push(`shim: ok -> ${target}`);
    }
  }

  const stampRead = await readInstallStamp(runtime);
  if (stampRead.status !== "valid") {
    issues.push({
      component: "runtime",
      fix: installFix,
      message: `runtime stamp is ${stampRead.status}`,
    });
  } else {
    checks.push(
      `runtime: ok ${stampRead.stamp.version} ${stampRead.stamp.commit} ${stampRead.stamp.installedAt}`,
    );
  }

  const recordRead = await readSourceRecord(home);
  if (recordRead.status !== "valid" || !existsSync(recordRead.record.path)) {
    issues.push({
      component: "source",
      fix: installFix,
      message: recordRead.status === "valid"
        ? `recorded source is missing: ${recordRead.record.path}`
        : `source record is ${recordRead.status}`,
    });
  } else {
    const [head, state] = await Promise.all([
      readGitHeadCommit(recordRead.record.path),
      command(recordRead.record.path, ["git", "status", "--porcelain", "--untracked-files=no"]),
    ]);
    if (!head || state.exitCode !== 0) {
      issues.push({
        component: "source",
        fix: installFix,
        message: `recorded source is not a readable git checkout: ${recordRead.record.path}`,
      });
    } else {
      checks.push(`source: ok ${recordRead.record.path} ${head} ${state.stdout ? "dirty" : "clean"}`);
    }
  }

  const wiringPaths = [
    join(repo, ".claude", "hooks", "maestro-record.ts"),
    join(repo, ".codex", "hooks", "maestro-record.ts"),
  ];
  for (const path of wiringPaths) {
    if (!existsSync(path)) {
      issues.push({ component: "wiring", fix: "run maestro install", message: `missing hook: ${path}` });
    }
  }
  for (const path of [join(repo, ".claude", "settings.json"), join(repo, ".codex", "hooks.json")]) {
    const settings = await readJsonObject(path);
    if (!settings || !JSON.stringify(settings).includes("maestro-record.ts")) {
      issues.push({ component: "wiring", fix: "run maestro install", message: `missing managed settings in ${path}` });
    }
  }
  for (const path of [join(repo, "AGENTS.md"), join(repo, "CLAUDE.md")]) {
    const text = existsSync(path) ? await readFile(path, "utf8") : "";
    if (!text.includes("<!-- maestro:begin -->")) {
      issues.push({ component: "wiring", fix: "run maestro install", message: `missing mirror block in ${path}` });
    }
  }
  const policy = await readJsonObject(join(repo, ".maestro", "config"));
  const ignore = join(repo, ".maestro", ".gitignore");
  if (!policy || !JSON.stringify(policy).includes("policy-proof")) {
    issues.push({ component: "wiring", fix: "run maestro install", message: "missing managed plugin config" });
  }
  if (!existsSync(ignore) || !(await readFile(ignore, "utf8")).includes("# maestro-ts:begin")) {
    issues.push({ component: "wiring", fix: "run maestro install", message: "missing managed .maestro/.gitignore block" });
  }
  if (!issues.some((issue) => issue.component === "wiring")) checks.push("wiring: ok");
  checks.push(await codexTrustCheck(repo));

  const storePath = resolveStoreLocation(repo).path;
  if (!existsSync(storePath)) {
    issues.push({ component: "store", fix: "run maestro install", message: `store is missing: ${storePath}` });
  } else {
    try {
      const database = new Database(storePath, { readonly: true, strict: true });
      const tables = database
        .query<{ count: number }, []>(
          "SELECT count(*) AS count FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .get();
      database.close();
      checks.push(`store: ok (${tables?.count ?? 0} tables)`);
    } catch (error) {
      issues.push({
        component: "store",
        fix: "restore store access, then run maestro doctor",
        message: `store is inaccessible: ${error instanceof Error ? error.message : String(error)}`,
      });
    }
  }

  if (issues.length > 0) {
    throw new CliError(
      "DOCTOR_ISSUES",
      issues.map((issue) => `${issue.component}: ${issue.message}; fix: ${issue.fix}`).join(" | "),
      { checks, issues },
    );
  }
  return { data: { checks, healthy: true }, text: `doctor: healthy\n${checks.join("\n")}` };
}

async function uninstall(): Promise<CliResult> {
  const repo = resolve(process.cwd());
  const removed = await uninstallRepo(repo);
  return {
    data: { removed },
    text: removed.length > 0
      ? `maestro uninstall removed ${removed.length} managed item${removed.length === 1 ? "" : "s"}`
      : "maestro uninstall: no changes",
  };
}

function registerLifecycle(cli: Cli): void {
  cli.register("doctor", doctor, {
    description: "Diagnose the machine runtime and current repository wiring read-only.",
  });
  cli.register("uninstall", uninstall, {
    description: "Remove Maestro-managed wiring from the current repository.",
  });
  cli.register("update", update, {
    description: "Fast-forward the recorded source checkout and resync the runtime.",
  });
}

export async function runLifecycleCommand(args: string[]): Promise<number | null> {
  if (!new Set(["doctor", "uninstall", "update"]).has(args[0] ?? "")) return null;
  const cli = new Cli();
  registerLifecycle(cli);
  return cli.dispatch(args);
}

export const lifecyclePlugin: BuiltInPlugin = {
  name: "lifecycle",
  apply(context) {
    for (const [verb, handler, description] of [
      ["doctor", doctor, "Diagnose the machine runtime and current repository wiring read-only."],
      ["uninstall", uninstall, "Remove Maestro-managed wiring from the current repository."],
      ["update", update, "Fast-forward the recorded source checkout and resync the runtime."],
    ] as const) {
      context.effect(() => context.cli.register(verb, handler, { description }));
    }
  },
};
