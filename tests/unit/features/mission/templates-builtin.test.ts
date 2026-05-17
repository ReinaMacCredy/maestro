import { describe, expect, it } from "bun:test";
import { BUILTIN_TEMPLATES } from "@/features/mission/templates/builtin.js";

describe("built-in mission templates", () => {
  it("ships exactly the four canonical templates", () => {
    const names = BUILTIN_TEMPLATES.map((t) => t.name).sort();
    expect(names).toEqual(["bug", "feature", "migration", "refactor"]);
  });

  it("each built-in template has at least one seed task and unique slugs", () => {
    for (const tpl of BUILTIN_TEMPLATES) {
      expect(tpl.seedTasks.length).toBeGreaterThan(0);
      const slugs = tpl.seedTasks.map((t) => t.slug);
      const unique = new Set(slugs);
      expect(unique.size).toBe(slugs.length);
    }
  });

  it("each built-in seed task has non-empty title + slug", () => {
    for (const tpl of BUILTIN_TEMPLATES) {
      for (const task of tpl.seedTasks) {
        expect(task.title.length).toBeGreaterThan(0);
        expect(task.slug.length).toBeGreaterThan(0);
        expect(task.slug).toMatch(/^[a-z][a-z0-9-]*[a-z0-9]$/);
      }
    }
  });

  it("marks each built-in template as source: builtin", () => {
    for (const tpl of BUILTIN_TEMPLATES) {
      expect(tpl.source).toBe("builtin");
    }
  });
});
