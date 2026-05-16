// Profile per-feature import cost. Run as:
//   bun /Users/reinamaccredy/Code/maestro/scripts/profile-imports.ts
//
// Each row is "<feature>  <ms>" measuring the wall-clock cost of importing
// that feature's index.ts the first time. Earlier imports also pay the cost
// of any shared dep (e.g. node:path) that hasn't been touched yet, so the
// order of measurement matters.

const features = [
  "@/shared/version-format.js",
  "@/shared/version.js",
  "@/shared/errors.js",
  "@/shared/lib/fs.js",
  "@/shared/lib/project-root.js",
  "@/shared/lib/deprecated-version-flag.js",
  "@/services.js",
  "@/infra/usecases/check-for-update.usecase.js",
  "@/infra/commands/init.command.js",
  "@/infra/commands/status.command.js",
  "@/infra/commands/doctor.command.js",
  "@/infra/commands/install.command.js",
  "@/infra/commands/update.command.js",
  "@/infra/commands/uninstall.command.js",
  "@/infra/commands/providers.command.js",
  "@/infra/commands/mission-control.command.js",
  "@/infra/usecases/install-release-binary.usecase.js",
  "@/features/notes/index.js",
  "@/features/session/index.js",
  "@/features/mission/index.js",
  "@/features/memory/index.js",
  "@/features/memory-ratchet/index.js",
  "@/features/graph/index.js",
  "@/features/task/index.js",
  "@/features/bundle/index.js",
  "@/features/evidence/index.js",
  "@/features/spec/index.js",
  "@/features/task/commands/contract-l2.command.js",
  "@/features/policy/commands/policy.command.js",
  "@/features/verdict/index.js",
  "@/features/plan/index.js",
  "@/features/ci/index.js",
  "@/features/review/index.js",
  "@/features/merge/index.js",
  "@/features/deploy/index.js",
  "@/features/runtime/index.js",
  "@/features/skills/index.js",
  "commander",
];

const results: Array<{ name: string; ms: number }> = [];
for (const name of features) {
  const t0 = performance.now();
  await import(name);
  const t1 = performance.now();
  results.push({ name, ms: t1 - t0 });
}

results.sort((a, b) => b.ms - a.ms);
for (const r of results) {
  console.log(`${r.ms.toFixed(2).padStart(8)}ms  ${r.name}`);
}
const total = results.reduce((s, r) => s + r.ms, 0);
console.log(`${total.toFixed(2).padStart(8)}ms  TOTAL`);
