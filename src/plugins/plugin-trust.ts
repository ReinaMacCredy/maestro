import { createHash } from "node:crypto";
import { chmod, lstat, mkdir, readdir, readFile, realpath, writeFile } from "node:fs/promises";
import { basename, dirname, join, relative } from "node:path";
import { CliError, type CliInvocation, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginRecord, PluginSource } from "../kernel/loader.ts";
import { registerSessionCommand } from "./session-required.ts";

export const trustFile = "trust.json";

export interface TrustGrant {
  digest: string;
  path: string;
  source: PluginSource;
  trustedAt: string;
}

export interface PluginArtifact {
  root: string;
  source: PluginSource;
}

export function trustPath(home: string): string {
  return join(home, ".maestro", trustFile);
}

// Fail closed: an unreadable or malformed trust file grants nothing rather than
// failing the whole command, so a corrupt file cannot be used to force a load.
export async function readTrust(home: string): Promise<TrustGrant[]> {
  let text: string;
  try {
    text = await readFile(trustPath(home), "utf8");
  } catch {
    return [];
  }
  try {
    const value = JSON.parse(text) as { plugins?: unknown };
    if (!Array.isArray(value.plugins)) return [];
    return value.plugins.filter((grant): grant is TrustGrant => {
      const candidate = grant as Partial<TrustGrant>;
      return typeof candidate.digest === "string" && typeof candidate.path === "string";
    });
  } catch {
    return [];
  }
}

export async function writeTrust(home: string, grants: TrustGrant[]): Promise<void> {
  const path = trustPath(home);
  await mkdir(dirname(path), { recursive: true });
  await chmod(dirname(path), 0o700);
  await writeFile(path, `${JSON.stringify({ plugins: grants }, null, 2)}\n`);
  await chmod(path, 0o600);
}

// The digest covers every regular file in the artifact, not just the entrypoint:
// index.ts is free to import a sibling or read a data file, so hashing the
// entrypoint alone would let a replacement change behavior without changing the
// digest. A symlink or special file anywhere inside makes the artifact
// undigestable, because its bytes live outside the tree being hashed.
export async function artifactDigest(root: string): Promise<string | null> {
  const files: string[] = [];
  const walk = async (current: string): Promise<boolean> => {
    const stats = await lstat(current);
    if (stats.isSymbolicLink()) return false;
    if (stats.isFile()) {
      files.push(current);
      return true;
    }
    if (!stats.isDirectory()) return false;
    for (const entry of await readdir(current)) {
      if (!(await walk(join(current, entry)))) return false;
    }
    return true;
  };
  try {
    if (!(await walk(root))) return null;
  } catch {
    return null;
  }
  const hash = createHash("sha256");
  for (const file of files.sort()) {
    const bytes = await readFile(file);
    hash.update(relative(root, file));
    hash.update("\0");
    hash.update(String(bytes.byteLength));
    hash.update("\0");
    hash.update(bytes);
  }
  return `sha256:${hash.digest("hex")}`;
}

// Key on the artifact's real location so the same file reached through a
// symlinked parent (/var vs /private/var on macOS, a symlinked checkout) is one
// artifact. The artifact itself is deliberately not followed: artifactDigest
// refuses a symlinked root, because its bytes would live outside the hash.
async function canonicalRoot(root: string): Promise<string> {
  return join(await realpath(dirname(root)), basename(root));
}

export function pluginTrustPredicate(
  home: string,
): (artifact: PluginArtifact) => Promise<boolean> {
  return async ({ root, source }) => {
    const digest = await artifactDigest(root);
    if (!digest) return false;
    const absolute = await canonicalRoot(root);
    return (await readTrust(home)).some(
      (grant) => grant.path === absolute && grant.digest === digest && grant.source === source,
    );
  };
}

export async function grantTrust(home: string, artifact: PluginArtifact): Promise<string> {
  const digest = await artifactDigest(artifact.root);
  if (!digest) {
    throw new CliError(
      "UNTRUSTABLE_PLUGIN",
      `plugin source cannot be digested (a symlink or special file inside it): ${artifact.root}`,
      { path: artifact.root },
    );
  }
  const absolute = await canonicalRoot(artifact.root);
  const grants = (await readTrust(home)).filter((grant) => grant.path !== absolute);
  grants.push({ digest, path: absolute, source: artifact.source, trustedAt: new Date().toISOString() });
  await writeTrust(home, grants);
  return digest;
}

// One-time migration, gated on the trust file's absence so it never resurrects
// a grant the user withdrew. Only ~/.maestro/plugins is grandfathered: a clone
// cannot write there, so everything in it arrived by plugin add or the user's
// own hand and already executes today. A <repo>/.maestro/plugins entry is
// exactly the clone-supplied artifact being defended against, so grandfathering
// it would defeat the boundary on day one for anyone who already cloned a
// hostile repository.
export async function grandfatherHomePlugins(home: string): Promise<string[]> {
  const directory = join(home, ".maestro", "plugins");
  let entries: string[];
  try {
    await lstat(trustPath(home));
    return [];
  } catch {
    // No trust file: this is the first run after the boundary landed.
  }
  try {
    entries = await readdir(directory);
  } catch {
    return [];
  }
  const grants: TrustGrant[] = [];
  const names: string[] = [];
  for (const entry of entries.sort()) {
    const root = join(directory, entry);
    const digest = await artifactDigest(root);
    if (!digest) continue;
    grants.push({
      digest,
      path: await canonicalRoot(root),
      source: "global",
      trustedAt: new Date().toISOString(),
    });
    names.push(entry.replace(/\.ts$/, ""));
  }
  await writeTrust(home, grants);
  return names;
}

export async function revokeTrust(home: string, root: string): Promise<boolean> {
  const absolute = await canonicalRoot(root).catch(() => root);
  const grants = await readTrust(home);
  const kept = grants.filter((grant) => grant.path !== absolute);
  if (kept.length === grants.length) return false;
  await writeTrust(home, kept);
  return true;
}

function untrustedRecord(records: readonly PluginRecord[], name: string): PluginRecord {
  const record = records.find((candidate) => candidate.name === name);
  if (!record) {
    throw new CliError("PLUGIN_SOURCE_NOT_FOUND", `plugin source not found: ${name}`, {
      plugin: name,
    });
  }
  if (!record.root) {
    throw new CliError("NOT_EXTERNAL_PLUGIN", `built-in plugins carry no trust record: ${name}`, {
      plugin: name,
    });
  }
  return record;
}

function requireName(invocation: CliInvocation): string {
  const value = invocation.positionals[0];
  if (!value) throw new CliError("MISSING_ARGUMENT", "missing plugin name");
  return value;
}

export const pluginTrustPlugin: BuiltInPlugin = {
  name: "plugin-trust",
  apply(context) {
    const home = process.env.HOME ?? process.cwd();

    // Without this line an existing user's plugin simply stops working with no
    // stated reason. It is stderr and never a prompt: the caller that most
    // often reaches here is a non-interactive harness hook, which a prompt
    // would hang.
    const untrusted = context.loader.records.filter((record) => record.status === "untrusted");
    if (untrusted.length > 0) {
      const named = untrusted
        .map((record) => `${record.name} (${record.source}, ${record.path})`)
        .join(", ");
      process.stderr.write(
        `[plugin] ${untrusted.length} untrusted plugin(s) not loaded: ${named}; ` +
          `review, then: maestro plugin trust ${untrusted[0]?.name}\n`,
      );
    }

    context.effect(() =>
      registerSessionCommand(context, "plugin trust", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        const record = untrustedRecord(context.loader.records, name);
        const digest = await grantTrust(home, {
          root: record.root as string,
          source: record.source,
        });
        context.log.append({
          type: "plugin.trust",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
          payload: { digest, path: record.root },
        });
        return {
          data: { digest, name, path: record.root },
          text: `${name} trusted\n${record.root}\n${digest}`,
        };
      }, {
        description: "Trust an external plugin's current source so it may load.",
        positionals: [{ name: "name", required: true }],
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "plugin untrust", async (invocation): Promise<CliResult> => {
        const name = requireName(invocation);
        const record = untrustedRecord(context.loader.records, name);
        if (!(await revokeTrust(home, record.root as string))) {
          throw new CliError("NOT_TRUSTED", `plugin is not trusted: ${name}`, { plugin: name });
        }
        context.log.append({
          type: "plugin.untrust",
          entityType: "plugin",
          entityId: name,
          sessionId: context.sessions.current().id,
        });
        return { data: { name }, text: `${name} untrusted` };
      }, {
        description: "Withdraw an external plugin's trust so it stops loading.",
        positionals: [{ name: "name", required: true }],
      }),
    );
  },
};
