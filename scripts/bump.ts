/**
 * Bump the patch version in package.json and src/version.ts.
 * Usage: bun scripts/bump.ts [major|minor|patch]
 * Default: patch
 */
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const pkgPath = join(root, "package.json");
const versionPath = join(root, "src", "version.ts");

const part = (process.argv[2] ?? "patch") as "major" | "minor" | "patch";
if (!["major", "minor", "patch"].includes(part)) {
  console.error(`[!] Invalid part: ${part}. Use major, minor, or patch.`);
  process.exit(1);
}

const pkg = await Bun.file(pkgPath).json();
const [major, minor, patch] = pkg.version.split(".").map(Number) as [number, number, number];

let next: string;
switch (part) {
  case "major": next = `${major + 1}.0.0`; break;
  case "minor": next = `${major}.${minor + 1}.0`; break;
  case "patch": next = `${major}.${minor}.${patch + 1}`; break;
}

pkg.version = next;
await Bun.write(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
await Bun.write(versionPath, `export const VERSION = "${next}";\n`);

console.log(`[ok] ${pkg.version.replace(next, `${major}.${minor}.${patch}`)} --> ${next}`);
