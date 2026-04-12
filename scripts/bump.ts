/**
 * Bump the version in package.json and src/shared/version.ts.
 * Scheme: 0.x.y where x = feature, y = patch.
 * Usage: bun scripts/bump.ts [feature|patch]
 * Default: patch
 */
import { join } from "node:path";
import { parseReleaseVersion, writeVersionArtifacts } from "./version-file";

const root = join(import.meta.dir, "..");
const pkgPath = join(root, "package.json");
const versionPath = join(root, "src", "shared", "version.ts");

const part = (process.argv[2] ?? "patch") as "feature" | "patch";
if (!["feature", "patch"].includes(part)) {
  console.error(`[!] Invalid part: ${part}. Use feature or patch.`);
  process.exit(1);
}

const pkg = await Bun.file(pkgPath).json();
const { feature: x, patch: y } = parseReleaseVersion(pkg.version);

const next = part === "feature"
  ? `0.${x + 1}.0`
  : `0.${x}.${y + 1}`;

await writeVersionArtifacts({
  cwd: root,
  pkgPath,
  versionPath,
  pkg,
  version: next,
});

console.log(`[ok] ${pkg.version} --> ${next}`);
