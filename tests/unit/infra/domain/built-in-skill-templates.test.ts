import { describe, expect, it } from "bun:test";
import { BUILT_IN_SKILL_TEMPLATES } from "@/infra/domain/built-in-skill-templates.js";

describe("BUILT_IN_SKILL_TEMPLATES", () => {
  it("is empty in v2 — the colon-tier built-in skill set was retired", () => {
    expect(BUILT_IN_SKILL_TEMPLATES).toEqual([]);
  });
});
