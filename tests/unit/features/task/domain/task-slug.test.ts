import { describe, expect, it } from "bun:test";
import {
  ALLOWED_VERBS,
  deriveSlugFromTitle,
  isValidSlugShape,
  kebabFromTitle,
  parseSlug,
} from "@/features/task/domain/task-slug.js";
import { MaestroError } from "@/shared/errors.js";

describe("isValidSlugShape", () => {
  it("accepts a kebab slug under an allowed verb", () => {
    expect(isValidSlugShape("implement/template-prompt-fixes")).toBe(true);
    expect(isValidSlugShape("fix/race")).toBe(true);
    expect(isValidSlugShape("chore/release")).toBe(true);
    expect(isValidSlugShape("spike/probe")).toBe(true);
    expect(isValidSlugShape("epic/big")).toBe(true);
  });

  it("rejects unknown verbs", () => {
    expect(isValidSlugShape("foo/bar")).toBe(false);
    expect(isValidSlugShape("refactor/x")).toBe(false);
  });

  it("rejects malformed slugs", () => {
    expect(isValidSlugShape("implement")).toBe(false);
    expect(isValidSlugShape("implement/")).toBe(false);
    expect(isValidSlugShape("/foo")).toBe(false);
    expect(isValidSlugShape("implement/Foo")).toBe(false); // uppercase
    expect(isValidSlugShape("implement/foo bar")).toBe(false); // spaces
    expect(isValidSlugShape("implement/-foo")).toBe(false); // leading dash on tail
    expect(isValidSlugShape("implement/foo--bar")).toBe(false); // double dash
    expect(isValidSlugShape("implement/foo/bar")).toBe(false); // extra slash
  });

  it("rejects total length over 60 chars", () => {
    const long = `implement/${"a".repeat(60)}`;
    expect(isValidSlugShape(long)).toBe(false);
  });
});

describe("kebabFromTitle", () => {
  it("kebabs basic ASCII", () => {
    expect(kebabFromTitle("Hello World")).toBe("hello-world");
  });

  it("transliterates non-ASCII", () => {
    expect(kebabFromTitle("Café au lait")).toBe("cafe-au-lait");
  });

  it("strips punctuation, collapses dashes", () => {
    expect(kebabFromTitle("Foo, Bar -- Baz!")).toBe("foo-bar-baz");
  });

  it("truncates at the configured cap", () => {
    const result = kebabFromTitle("a".repeat(80), 30);
    expect(result.length).toBeLessThanOrEqual(30);
  });

  it("trims trailing dashes after truncation", () => {
    const result = kebabFromTitle("a-b-c-d-e-f-g-h-i-j-k-l-m-n-o-p", 5);
    expect(result.endsWith("-")).toBe(false);
  });
});

describe("deriveSlugFromTitle", () => {
  it("uses 'implement' for feature/task by default", () => {
    expect(deriveSlugFromTitle("Add login form", "feature")).toBe("implement/add-login-form");
    expect(deriveSlugFromTitle("Refresh queue", "task")).toBe("implement/refresh-queue");
  });

  it("uses 'fix' for bugs", () => {
    // "in" is a stop word; the derived slug drops it.
    expect(deriveSlugFromTitle("Race in writer", "bug")).toBe("fix/race-writer");
  });

  it("uses 'chore' for chores", () => {
    expect(deriveSlugFromTitle("Bump deps", "chore")).toBe("chore/bump-deps");
  });

  it("uses 'epic' for epics", () => {
    expect(deriveSlugFromTitle("Mission alpha", "epic")).toBe("epic/mission-alpha");
  });

  it("caps at 4 significant words and ~32-char tail for readability", () => {
    const slug = deriveSlugFromTitle(
      "Check maestro task dependency support against beads-rust",
      "chore",
    );
    expect(slug).toBe("chore/check-maestro-task-dependency");
    expect(slug.length).toBeLessThanOrEqual(40);
  });

  it("drops hex commit hashes and pure-digit tokens", () => {
    const slug = deriveSlugFromTitle(
      "Read-only standards review 35f644fd^..1c8296da",
      "task",
    );
    expect(slug).toBe("implement/read-only-standards-review");
  });

  it("never truncates mid-word", () => {
    const slug = deriveSlugFromTitle(
      "Build and verify compact task ready through dist and install",
      "chore",
    );
    expect(slug.endsWith("-")).toBe(false);
    for (const part of slug.slice("chore/".length).split("-")) {
      // each token in the kebab tail is a complete English-style word, not a
      // mid-word fragment like "ru" (truncated "rust").
      expect(part).toMatch(/^[a-z0-9]+$/);
    }
  });

  it("respects 60-char cap when title is long", () => {
    const long = "a ".repeat(60);
    const slug = deriveSlugFromTitle(long, "feature");
    expect(slug.length).toBeLessThanOrEqual(60);
    expect(isValidSlugShape(slug)).toBe(true);
  });

  it("throws when title cannot derive any kebab", () => {
    expect(() => deriveSlugFromTitle("   ", "feature")).toThrow(MaestroError);
    expect(() => deriveSlugFromTitle("!@#$%", "feature")).toThrow(MaestroError);
  });
});

describe("parseSlug", () => {
  it("returns the slug verbatim when valid", () => {
    expect(parseSlug("implement/foo")).toBe("implement/foo");
  });

  it("throws on invalid shape", () => {
    expect(() => parseSlug("Foo/Bar")).toThrow(MaestroError);
    expect(() => parseSlug("notaverb/foo")).toThrow(MaestroError);
  });
});

describe("ALLOWED_VERBS", () => {
  it("exposes the canonical list", () => {
    expect(ALLOWED_VERBS).toEqual(["implement", "fix", "chore", "spike", "epic"]);
  });
});
