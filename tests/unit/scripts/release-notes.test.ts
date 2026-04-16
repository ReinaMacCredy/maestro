import { describe, expect, it } from "bun:test";
import {
  buildReleaseNotes,
  extractChangelogSection,
  renderFallbackReleaseNotes,
} from "../../../scripts/release-notes-lib";

describe("extractChangelogSection", () => {
  it("extracts a release section with a hyphen heading", () => {
    const changelog = `# Changelog

## 0.37.4 - Inline GitHub release notes

- First change

## 0.37.3 - Previous release

- Older change
`;

    expect(extractChangelogSection(changelog, "0.37.4")).toBe(
      "## 0.37.4 - Inline GitHub release notes\n\n- First change\n",
    );
  });

  it("extracts a release section with an em dash heading", () => {
    const changelog = `# Changelog

## 0.35.4 — Typecheck and release metadata alignment

- First change
`;

    expect(extractChangelogSection(changelog, "0.35.4")).toBe(
      "## 0.35.4 — Typecheck and release metadata alignment\n\n- First change\n",
    );
  });
});

describe("renderFallbackReleaseNotes", () => {
  it("renders commit bullets with the previous tag context", () => {
    expect(renderFallbackReleaseNotes({
      version: "0.37.4",
      previousTag: "v0.37.3",
      commitSubjects: [
        "fix(release): show inline notes",
        "test(release): cover changelog fallback",
      ],
    })).toBe(
      "## 0.37.4\n\nChanges since v0.37.3.\n\n- fix(release): show inline notes\n- test(release): cover changelog fallback\n",
    );
  });

  it("renders a metadata fallback when there are no commit subjects", () => {
    expect(renderFallbackReleaseNotes({
      version: "0.37.4",
      commitSubjects: [],
    })).toBe(
      "## 0.37.4\n\nChanges in this release.\n\n- Release metadata update.\n",
    );
  });
});

describe("buildReleaseNotes", () => {
  it("prefers the changelog section over fallback commit bullets", () => {
    const changelog = `# Changelog

## 0.37.4 - Inline GitHub release notes

- First change
`;

    expect(buildReleaseNotes({
      version: "0.37.4",
      changelog,
      previousTag: "v0.37.3",
      commitSubjects: ["fix(release): show inline notes"],
    })).toBe(
      "## 0.37.4 - Inline GitHub release notes\n\n- First change\n",
    );
  });
});
