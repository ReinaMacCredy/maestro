import { describe, it, expect } from "bun:test";
import { InvalidArgumentError } from "commander";
import { singletonOption } from "@/shared/lib/cli-options.js";

describe("singletonOption", () => {
  it("returns the value when previous is undefined (first use)", () => {
    expect(singletonOption("tsk-abc", undefined)).toBe("tsk-abc");
  });

  it("throws InvalidArgumentError when previous is a string (second use)", () => {
    expect(() => singletonOption("tsk-def", "tsk-abc")).toThrow(InvalidArgumentError);
  });

  it("throws even when the second value matches the first (same-value smell)", () => {
    expect(() => singletonOption("tsk-abc", "tsk-abc")).toThrow(InvalidArgumentError);
  });

  it("error message says pass it once", () => {
    expect(() => singletonOption("anything", "anything")).toThrow("pass it once, not multiple times");
  });

  it("treats any defined previous as already-set (including empty string)", () => {
    expect(() => singletonOption("second", "")).toThrow(InvalidArgumentError);
  });
});
