import { Database } from "bun:sqlite";
import { existsSync } from "node:fs";
import { cp, mkdir, readFile, rename, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { Cli, CliError, type CliOptions, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { resolveStoreLocation } from "../kernel/store.ts";
import { warnBeforeRuntimeActivation } from "./activation-scan.ts";
import {
  codexHookTrustRecorded,
  forgetRepository,
  readGitHeadCommit,
  gitMainWorktree,
  stampRuntime,
  uninstallRepo,
} from "./install.ts";
import { readInstallStamp } from "./install-stamp.ts";
import { resolveHomeDirectory } from "./home.ts";
import { grandfatherHomePlugins } from "./plugin-trust.ts";
import { installInRoomMessage, isRoom } from "./room.ts";
import { skillNames } from "./skills.ts";
import { removeRenderedProfiles } from "./profiles.ts";
import { unlinkHerdrPlugin } from "./slp-plugin.ts";
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
  return resolveHomeDirectory();
}

function runtimeRoot(home: string): string {
  return join(home, ".maestro", "runtime");
}

function cwdIsRoom(cwd: string): boolean {
  let database: Database | null = null;
  try {
    const store = resolveStoreLocation(cwd);
    if (!existsSync(store.path)) return false;
    database = new Database(store.path, { readonly: true, strict: true });
    return isRoom(database);
  } catch {
    return false;
  } finally {
    database?.close();
  }
}

function refuseRepositoryWiringInRoom(cwd: string): void {
  if (cwdIsRoom(cwd)) throw new CliError("INSTALL_IN_ROOM", installInRoomMessage);
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

// scripts/install.sh leaves an adopter's checkout on this branch, sitting at a
// release tag with no upstream (d714).
const pinnedBranch = "maestro-release";

// Version order, not string order: v0.9.0 outranks v0.10.0 lexicographically.
export function newestReleaseTag(tags: readonly string[]): string | null {
  let best: { tag: string; value: number } | null = null;
  for (const tag of tags) {
    const parts = /^v(\d+)\.(\d+)\.(\d+)$/.exec(tag);
    if (!parts) continue;
    const value = Number(parts[1]) * 1_000_000 + Number(parts[2]) * 1_000 + Number(parts[3]);
    if (!best || value > best.value) best = { tag, value };
  }
  return best?.tag ?? null;
}

async function fastForwardToNewestTag(source: string): Promise<void> {
  const fetched = await command(source, ["git", "fetch", "--tags", "origin"]);
  if (fetched.exitCode !== 0) {
    throw new CliError(
      "UPDATE_FETCH_FAILED",
      `cannot fetch release tags for the Maestro source: ${fetched.stderr || "git fetch failed"}; fix remote connectivity, then run maestro update`,
      { fix: "fix the source remote or network, then run maestro update" },
    );
  }
  const listed = await command(source, ["git", "tag", "--list"]);
  const tag = newestReleaseTag(listed.stdout.split("\n").map((line) => line.trim()));
  if (!tag) {
    throw new CliError(
      "UPDATE_NO_RELEASE_TAG",
      `the Maestro source on ${pinnedBranch} has no release tag to follow; check out a branch that tracks an upstream, then run maestro update`,
      { fix: "check out a tracking branch, then run maestro update" },
    );
  }
  const merged = await command(source, ["git", "merge", "--ff-only", tag]);
  if (merged.exitCode !== 0) {
    throw new CliError(
      "UPDATE_MERGE_FAILED",
      `cannot fast-forward the pinned Maestro source to ${tag}: ${merged.stderr || "git merge --ff-only failed"}; fix the checkout, then run maestro update`,
      { fix: "fix the source checkout, then run maestro update" },
    );
  }
}

async function update(): Promise<CliResult> {
  refuseRepositoryWiringInRoom(resolve(process.cwd()));
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
  // Untracked files outside the runtime surface are ignored: install itself
  // leaves untracked wiring (.claude/, .codex/) in a wired source checkout.
  // An untracked file under the surface that syncRuntime copies would ship
  // in a runtime stamped as HEAD, so it counts as dirt.
  const status = await command(source, ["git", "status", "--porcelain", "--untracked-files=all"]);
  const runtimeSurface = /^(package\.json|tsconfig\.json|bin\/|src\/)/;
  const dirt = status.stdout
    .split("\n")
    .filter((line) => line.length > 3)
    .filter((line) => !line.startsWith("??") || runtimeSurface.test(line.slice(3)));
  if (status.exitCode !== 0 || dirt.length > 0) {
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
  // A checkout scripts/install.sh pinned follows the release tag line, not a
  // branch tip, and has no upstream to pull from (d714). The branch name is the
  // signal: right after a release, main and the newest tag are the same commit,
  // so HEAD-sits-on-a-tag would misread a main checkout as pinned.
  const pinned = branch.stdout === pinnedBranch;
  if (pinned) await fastForwardToNewestTag(source);
  const upstream = pinned
    ? { exitCode: 1, stdout: "", stderr: "" }
    : await command(source, [
      "git",
      "rev-parse",
      "--abbrev-ref",
      "--symbolic-full-name",
      "@{upstream}",
    ]);
  // A branch nobody has pushed has nothing to pull, which is the same situation
  // the ahead-only path below already rules is not an error. Resync the runtime
  // and name the missing upstream so a real misconfiguration stays visible (d38).
  const noUpstream = !pinned && (upstream.exitCode !== 0 || !upstream.stdout);
  // Every lane branches inside this same checkout, so a branch no remote has
  // published is unreviewed code, and the drift nag that asks for the update
  // reads the same either way (d728). d38's ruling that a missing upstream is
  // not an error survives where it was written, a checkout with no remote.
  if (noUpstream && (await command(source, ["git", "remote"])).stdout) {
    throw new CliError(
      "UPDATE_SOURCE_UNPUBLISHED",
      `Maestro source is on ${branch.stdout}, a branch no remote has published; check out the branch the runtime follows, then run maestro update, or run maestro install from ${source} to install this branch on purpose`,
      { fix: `check out the branch the runtime follows in ${source}, then run maestro update` },
    );
  }
  const sourceBranch = noUpstream
    ? (await command(source, ["git", "rev-parse", "--abbrev-ref", "HEAD"])).stdout || "HEAD"
    : "";
  let aheadOnly = false;
  if (!pinned && !noUpstream) {
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
  }
  let staged: string | null = null;
  try {
    await warnBeforeRuntimeActivation(home, "update");
    staged = await stageRuntime(source, runtime);
    await swapRuntime(staged, runtime);
    staged = null;
    // Upgrading past the plugin trust boundary must not silently stop the
    // plugins the user already installed by hand; this is a no-op once the
    // trust file exists.
    await grandfatherHomePlugins(home);
  } catch (error) {
    const reset = await command(source, ["git", "reset", "--hard", oldCommit]);
    if (staged && existsSync(staged)) await rm(staged, { recursive: true, force: true });
    const currentCommit = await readGitHeadCommit(source);
    if (reset.exitCode !== 0 || currentCommit !== oldCommit) {
      const recoveryCommand = `git -C ${JSON.stringify(source)} reset --hard ${oldCommit}`;
      throw new CliError(
        "UPDATE_ROLLBACK_FAILED",
        `runtime resync failed and source rollback failed; old commit ${oldCommit}; current commit ${currentCommit ?? "unknown"}; reset: ${reset.stderr || `git exited ${reset.exitCode}`}; run: ${recoveryCommand}`,
        { currentCommit, oldCommit, recoveryCommand, resetStderr: reset.stderr },
      );
    }
    if (error instanceof CliError && error.code === "SLP_V1_TEAM_RUNNING") throw error;
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
  // Same reason as the room templates below: skillNames and the skill sources
  // live in the module this process imported before the swap, so materializing
  // in-process writes the outgoing release's list and a skill the new commit
  // adds needs a second update to appear.
  let skillText = "";
  if (newCommit) {
    const synced = await command(runtime, [
      process.execPath,
      "-e",
      `const skills = await import(${JSON.stringify(pathToFileURL(join(runtime, "src", "plugins", "skills.ts")).href)});` +
        `const sync = await skills.materializeSkills(${JSON.stringify(home)}, ${JSON.stringify(newCommit)});` +
        `console.log(skills.formatSkillSync(sync));`,
    ]);
    if (synced.exitCode !== 0) {
      throw new CliError(
        "SKILL_SYNC_FAILED",
        `runtime installed but the managed skills were not materialized: ${synced.stderr || `bun exited ${synced.exitCode}`}; run maestro update again`,
        { fix: "run maestro update again" },
      );
    }
    skillText = synced.stdout.trim();
  }
  // Refresh the generated room templates after an update; previously only
  // install did this. The templates are compiled into room.ts, and this
  // process imported it before the swap, so calling scaffoldRoom in-process
  // writes the outgoing release's text and leaves the room reading doctrine
  // one release behind. A query string does not help: Bun keys the module
  // cache on the resolved path and ignores it. Only a fresh process loads the
  // runtime that was just swapped in.
  const scaffolded = await command(runtime, [
    process.execPath,
    "-e",
    `const room = await import(${JSON.stringify(pathToFileURL(join(runtime, "src", "plugins", "room.ts")).href)});` +
      `await room.scaffoldRoom(${JSON.stringify(home)});` +
      // The rendered seat profiles follow the runtime the same way (Hub d83).
      `const profiles = await import(${JSON.stringify(pathToFileURL(join(runtime, "src", "plugins", "profiles.ts")).href)});` +
      `await profiles.materializeProfiles(${JSON.stringify(home)}, ${JSON.stringify(resolve(process.cwd()))});`,
  ]);
  if (scaffolded.exitCode !== 0) {
    throw new CliError(
      "ROOM_SCAFFOLD_FAILED",
      `runtime installed but the room templates were not refreshed: ${scaffolded.stderr || `bun exited ${scaffolded.exitCode}`}; run maestro update again`,
      { fix: "run maestro update again" },
    );
  }
  return {
    data: { aheadOnly, noUpstream, oldCommit, newCommit, version: packageJson.version },
    text: (noUpstream
      ? `${oldCommit} up to date (no upstream for ${sourceBranch}; nothing to pull)\nmaestro ${packageJson.version}`
      : aheadOnly
      ? `${oldCommit} up to date (source ahead of upstream; nothing to pull)\nmaestro ${packageJson.version}`
      : `${oldCommit} -> ${newCommit}\nmaestro ${packageJson.version}`) +
      (skillText ? `\n${skillText}` : ""),
  };
}

export async function driftAdvisory(_home: string, runningRoot: string): Promise<string> {
  if (process.env.MAESTRO_AUTO_UPDATE === "0") return "";
  const home = resolveHomeDirectory();
  const [stampRead, recordRead] = await Promise.all([
    readInstallStamp(runningRoot),
    readSourceRecord(home),
  ]);
  if (stampRead.status !== "valid" || recordRead.status !== "valid") return "";
  const sourceCommit = await readGitHeadCommit(recordRead.record.path);
  if (!sourceCommit || sourceCommit === stampRead.stamp.commit) return "";
  const source = recordRead.record.path;
  const branch = await command(source, ["git", "symbolic-ref", "--quiet", "--short", "HEAD"]);
  const where = branch.stdout ? `on ${branch.stdout}` : "on a detached HEAD";
  // The branch says which line the update would install; the count says how much
  // of it nobody else can see, which is the shape update still accepts on a
  // tracking branch that is only ahead (d728). Both are meaningless without a
  // remote to be unpublished against.
  const counted = (await command(source, ["git", "remote"])).stdout
    ? Number((await command(source, ["git", "rev-list", "--count", "HEAD", "--not", "--remotes"])).stdout)
    : 0;
  // A nag is advisory: a git call that fails here drops the count rather than
  // printing NaN beside a commit the reader is meant to trust.
  const unpublished = Number.isFinite(counted) ? counted : 0;
  const held = unpublished > 0
    ? ` (${unpublished} commit${unpublished === 1 ? "" : "s"} no remote holds)`
    : "";
  return `[update] runtime ${stampRead.stamp.commit.slice(0, 8)} differs from source ${sourceCommit.slice(0, 8)} ${where}${held}; run maestro update`;
}

async function readJsonObject(path: string): Promise<Record<string, unknown> | null> {
  try {
    return JSON.parse(await readFile(path, "utf8")) as Record<string, unknown>;
  } catch {
    return null;
  }
}

async function codexTrustCheck(repo: string, home: string): Promise<string> {
  const mainWorktree = await gitMainWorktree(repo);
  const hooks = join(mainWorktree ?? repo, ".codex", "hooks.json");
  if (!existsSync(hooks)) return "codex hooks: absent";
  const requiredHooks = [
    { event: "SessionStart", trust: "session_start" },
    { event: "UserPromptSubmit", trust: "user_prompt_submit" },
  ] as const;
  let declared: string[] = [];
  try {
    const wiring = JSON.parse(await readFile(hooks, "utf8")) as { hooks?: Record<string, unknown> };
    declared = Object.keys(wiring.hooks ?? wiring);
  } catch {
    declared = [];
  }
  const missing = requiredHooks.filter(({ event }) => !declared.includes(event));
  if (missing.length > 0) {
    return `codex hooks: stale (missing ${missing.map(({ event }) => event).join(", ")} in ${hooks}; run maestro install)`;
  }
  return await codexHookTrustRecorded(mainWorktree ?? repo, home)
    ? "codex hooks: recorded by Codex (both events trusted in ~/.codex/config.toml; hash not verifiable)"
    : "codex hooks: unverified (Codex trust hash contract unavailable; run /hooks in Codex to verify)";
}

// A repo migrating off the Rust build still carries .maestro/store.sqlite next to
// the new store. Nothing else mentions it, so a migrating user reads "no ready
// work" and concludes their history is gone (d35).
function legacyStoreCheck(storePath: string): string | null {
  const legacyPath = join(dirname(storePath), "store.sqlite");
  if (!existsSync(legacyPath)) return null;
  let legacy: Database | null = null;
  let imported: Database | null = null;
  try {
    legacy = new Database(legacyPath, { readonly: true, strict: true });
    const cards = legacy
      .query<{ count: number }, []>("SELECT count(*) AS count FROM cards")
      .get()?.count ?? 0;
    if (cards === 0) return null;
    if (existsSync(storePath)) {
      imported = new Database(storePath, { readonly: true, strict: true });
      const present = imported
        .query<{ name: string }, []>(
          "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'legacy_cards'",
        )
        .get();
      const alreadyImported = present
        ? (imported
          .query<{ count: number }, []>("SELECT count(*) AS count FROM legacy_cards")
          .get()?.count ?? 0)
        : 0;
      if (alreadyImported > 0) return `legacy store: imported (${alreadyImported} card(s))`;
    }
    return `legacy store: ${cards} card(s) not imported; run: maestro import rust`;
  } catch {
    return null;
  } finally {
    legacy?.close();
    imported?.close();
  }
}


async function doctor(): Promise<CliResult> {
  const home = homeDirectory();
  const repo = resolve(process.cwd());
  const runtime = runtimeRoot(home);
  const checks: string[] = [];
  const issues: DoctorIssue[] = [];
  const installFix = "run maestro install from the Maestro source checkout";
  const room = cwdIsRoom(repo);
  const roomFix =
    "run maestro install or maestro update from a registered repository checkout (registry lists them)";
  const now = new Date();
  const today = [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, "0"),
    String(now.getDate()).padStart(2, "0"),
  ].join("-");
  for (const name of skillNames) {
    const skill = join(home, "maestro", "skills", name, "SKILL.md");
    if (!existsSync(skill)) continue;
    const frontmatter = (await readFile(skill, "utf8")).match(
      /^---\r?\n([\s\S]*?)\r?\n---/,
    )?.[1];
    const reviewDate = frontmatter?.match(
      /^review-date:\s*(\d{4}-\d{2}-\d{2})\s*$/m,
    )?.[1];
    if (reviewDate && reviewDate < today) {
      issues.push({
        component: "skills",
        fix: "review the rule or move the date",
        message: `review date ${reviewDate} is past due: ${skill}`,
      });
    }
  }

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

  if (room) {
    let filesHealthy = true;
    for (const name of ["IDENTITY.md", "AGENTS.md", "CLAUDE.md", "SLP.md", "OWNER.md"]) {
      const path = join(repo, name);
      if (!existsSync(path)) {
        filesHealthy = false;
        issues.push({ component: "room", fix: roomFix, message: `missing room file: ${path}` });
      }
    }
    if (filesHealthy) checks.push("room files: ok");
    const registry = join(repo, "registry");
    if (existsSync(registry)) {
      checks.push("room registry: ok");
    } else {
      issues.push({ component: "room", fix: roomFix, message: `missing room registry: ${registry}` });
    }

    let hooksHealthy = true;
    for (const path of [
      join(repo, ".claude", "hooks", "maestro-record.ts"),
      join(repo, ".codex", "hooks", "maestro-record.ts"),
    ]) {
      if (!existsSync(path)) {
        hooksHealthy = false;
        issues.push({ component: "room", fix: roomFix, message: `missing room hook: ${path}` });
      }
    }
    for (const path of [
      join(repo, ".claude", "settings.json"),
      join(repo, ".codex", "hooks.json"),
    ]) {
      const settings = await readJsonObject(path);
      if (!settings || !JSON.stringify(settings).includes("maestro-record.ts")) {
        hooksHealthy = false;
        issues.push({
          component: "room",
          fix: roomFix,
          message: `missing room hook settings: ${path}`,
        });
      }
    }
    if (hooksHealthy) checks.push("room hooks: ok");
  } else {
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
    const policy = await readJsonObject(join(repo, ".maestro", "config"));
    const ignore = join(repo, ".maestro", ".gitignore");
    if (!policy || !JSON.stringify(policy).includes("policy-proof")) {
      issues.push({ component: "wiring", fix: "run maestro install", message: "missing managed plugin config" });
    }
    if (!existsSync(ignore) || !(await readFile(ignore, "utf8")).includes("# maestro-ts:begin")) {
      issues.push({ component: "wiring", fix: "run maestro install", message: "missing managed .maestro/.gitignore block" });
    }
    if (!issues.some((issue) => issue.component === "wiring")) checks.push("wiring: ok");
    checks.push(await codexTrustCheck(repo, home));
  }
  const roomRoot = room ? repo : join(home, "maestro");
  const roomSettings = await readJsonObject(join(roomRoot, ".claude", "settings.json"));
  const roomPermissions = roomSettings?.permissions;
  const roomDeny = roomPermissions && typeof roomPermissions === "object" &&
      !Array.isArray(roomPermissions)
    ? (roomPermissions as Record<string, unknown>).deny
    : null;
  const roomDenyHealthy = Array.isArray(roomDeny) &&
    roomDeny.includes("Agent") && roomDeny.includes("Task");
  checks.push(
    `room deny list: ${roomDenyHealthy ? "ok" : "missing"}`,
  );
  if (room && !roomDenyHealthy) {
    issues.push({ component: "room", fix: roomFix, message: "room deny list is missing" });
  }

  const storeLocation = resolveStoreLocation(repo);
  const storePath = storeLocation.path;
  if (storeLocation.orphanPath) {
    checks.push(`orphan store: ${storeLocation.orphanPath} left untouched`);
  }
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
      const legacy = legacyStoreCheck(storePath);
      if (legacy) checks.push(legacy);
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
  refuseRepositoryWiringInRoom(repo);
  const removed = await uninstallRepo(repo);
  removed.push(...await removeRenderedProfiles(homeDirectory()));
  removed.push(...await unlinkHerdrPlugin(homeDirectory()));
  if (await forgetRepository(homeDirectory(), repo)) removed.push(`${repo} registry line`);
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
    mutates: false,
  });
  cli.register("uninstall", uninstall, {
    description: "Remove Maestro-managed wiring from the current repository; to drop a repository from the Hub registry only, run maestro room forget <path>.",
  });
  cli.register("update", update, {
    description: "Fast-forward the recorded source checkout and resync the runtime.",
  });
}

export async function runLifecycleCommand(
  args: string[],
  cliOptions: CliOptions = {},
): Promise<number | null> {
  if (!new Set(["doctor", "uninstall", "update"]).has(args[0] ?? "")) return null;
  const cli = new Cli(cliOptions);
  registerLifecycle(cli);
  return cli.dispatch(args);
}

export const lifecyclePlugin: BuiltInPlugin = {
  name: "lifecycle",
  apply(context) {
    for (const [verb, handler, description] of [
      ["doctor", doctor, "Diagnose the machine runtime and current repository wiring read-only."],
      ["uninstall", uninstall, "Remove Maestro-managed wiring from the current repository; to drop a repository from the Hub registry only, run maestro room forget <path>."],
      ["update", update, "Fast-forward the recorded source checkout and resync the runtime."],
    ] as const) {
      context.effect(() =>
        context.cli.register(verb, handler, {
          description,
          ...(verb === "doctor" ? { mutates: false } : {}),
        })
      );
    }
  },
};
