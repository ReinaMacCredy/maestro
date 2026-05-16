/**
 * Bump the version in package.json and src/shared/version.ts.
 * Scheme: MAJOR.MINOR.PATCH.
 * Usage: bun scripts/bump.ts [major|feature|patch]
 * Default: patch
 */
import { join } from "node:path";
import { parseReleaseVersion, writeVersionArtifacts } from "./version-file";

const root = join(import.meta.dir, "..");
const pkgPath = join(root, "package.json");
const versionPath = join(root, "src", "shared", "version.ts");

const part = (process.argv[2] ?? "patch") as "major" | "feature" | "patch";
if (!["major", "feature", "patch"].includes(part)) {
  console.error(`[!] Invalid part: ${part}. Use major, feature, or patch.`);
  process.exit(1);
}

const pkg = await Bun.file(pkgPath).json();
const currentVersion: string = pkg.version;
const { major: m, feature: x, patch: y } = parseReleaseVersion(currentVersion);

const next = part === "major"
  ? `${m + 1}.0.0`
  : part === "feature"
    ? `${m}.${x + 1}.0`
    : `${m}.${x}.${y + 1}`;

await writeVersionArtifacts({
  cwd: root,
  pkgPath,
  versionPath,
  pkg,
  version: next,
});

console.log(`[ok] ${currentVersion} --> ${next}`);
