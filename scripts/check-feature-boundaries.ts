// Feature-folder boundary check. See AGENTS.md (Feature-Folder Layout)
// for the rule; this script enforces it on the repo's feature sources.
import {
  resolveBoundaryCheckRoot,
  scanFeatureBoundaryViolations,
} from "./check-feature-boundaries-lib";

const ROOT = resolveBoundaryCheckRoot(import.meta.url);
const violations = await scanFeatureBoundaryViolations(ROOT);

if (violations.length === 0) {
  console.log("Feature boundaries OK");
  process.exit(0);
}

console.error("[!] Feature boundary violations:\n");
for (const v of violations) {
  console.error(`  ${v.file}`);
  console.error(`    feature '${v.ownFeature}' deep-imports from '${v.otherFeature}'`);
  console.error(`    spec: ${v.importSpec}`);
  console.error("");
}
console.error(
  `Total: ${violations.length} violation(s). Cross-feature imports must go ` +
    `through the public surface (@/features/<name>) which resolves to index.ts.`,
);
process.exit(1);
