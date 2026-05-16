import { describe, expect, it } from "bun:test";
import { applyPathChanges } from "@/service/contract-helpers.js";

describe("applyPathChanges", () => {
  it("adds new literal paths and reports skip on exact duplicates", () => {
    const { result, skipped } = applyPathChanges(
      ["src/foo.ts"],
      ["src/bar.ts", "src/foo.ts"],
      [],
    );
    expect(result).toEqual(["src/foo.ts", "src/bar.ts"]);
    expect(skipped).toEqual(["src/foo.ts"]);
  });

  it("skips paths covered by an existing glob", () => {
    const { result, skipped } = applyPathChanges(
      ["src/**/*.ts"],
      ["src/foo.ts"],
      [],
    );
    expect(result).toEqual(["src/**/*.ts"]);
    expect(skipped).toEqual(["src/foo.ts"]);
  });

  it("is order-independent for mixed glob+literal batches", () => {
    const a = applyPathChanges([], ["foo.ts", "*.ts"], []);
    const b = applyPathChanges([], ["*.ts", "foo.ts"], []);
    expect(new Set(a.result)).toEqual(new Set(b.result));
    expect(a.skipped).toEqual([]);
    expect(b.skipped).toEqual([]);
  });

  it("removes paths from existing before evaluating adds", () => {
    const { result } = applyPathChanges(
      ["src/foo.ts", "src/bar.ts"],
      ["src/bar.ts"],
      ["src/foo.ts"],
    );
    expect(result).toEqual(["src/bar.ts"]);
  });

  it("deduplicates same-batch repeats silently", () => {
    const { result, skipped } = applyPathChanges([], ["foo.ts", "foo.ts"], []);
    expect(result).toEqual(["foo.ts"]);
    expect(skipped).toEqual([]);
  });
});
