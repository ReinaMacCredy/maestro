import type { MissionTemplate } from "../domain/template-types.js";

export const refactorTemplate: MissionTemplate = {
  name: "refactor",
  description: "Behavior-preserving code change. Tasks bias toward proving no regressions.",
  source: "builtin",
  seedTasks: [
    { title: "Survey current shape and call sites", slug: "survey" },
    { title: "Implement refactor", slug: "implement" },
    { title: "Add or extend regression tests", slug: "tests" },
    { title: "Verify no behavior change (build + full test pass)", slug: "verify" },
  ],
};

export const featureTemplate: MissionTemplate = {
  name: "feature",
  description: "New user-facing capability. Tasks cover design through verification.",
  source: "builtin",
  seedTasks: [
    { title: "Write feature spec / design note", slug: "spec" },
    { title: "Implement feature", slug: "implement" },
    { title: "Add tests (golden path + edge cases)", slug: "tests" },
    { title: "Verify end-to-end (run the feature, check side effects)", slug: "verify" },
  ],
};

export const bugTemplate: MissionTemplate = {
  name: "bug",
  description: "Fix a defect. Reproduce-first workflow; regression test is mandatory.",
  source: "builtin",
  seedTasks: [
    { title: "Reproduce the bug with a failing test", slug: "reproduce" },
    { title: "Localize root cause", slug: "localize" },
    { title: "Fix root cause (not the symptom)", slug: "fix" },
    { title: "Confirm regression test passes; run adjacent tests", slug: "verify" },
  ],
};

export const migrationTemplate: MissionTemplate = {
  name: "migration",
  description: "Schema, data, or config shape change. Tasks emphasize preflight + reversibility check.",
  source: "builtin",
  seedTasks: [
    { title: "Detect old shape and inventory affected records/files", slug: "detect" },
    { title: "Implement translation to new shape", slug: "translate" },
    { title: "Verify migration on a snapshot or test fixture", slug: "verify-snapshot" },
    { title: "Delete or quarantine old shape; document rollback", slug: "cleanup" },
  ],
};

export const BUILTIN_TEMPLATES: readonly MissionTemplate[] = [
  refactorTemplate,
  featureTemplate,
  bugTemplate,
  migrationTemplate,
];

export const BUILTIN_TEMPLATE_NAMES = BUILTIN_TEMPLATES.map((t) => t.name);
