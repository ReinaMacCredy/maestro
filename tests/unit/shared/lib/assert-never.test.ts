import { describe, it, expect } from "bun:test";
import { assertNever } from "@/shared/lib/assert-never.js";

describe("assertNever", () => {
  it("throws an error with the unexpected value", () => {
    const value = "unexpected" as never;
    expect(() => assertNever(value)).toThrow('Unexpected value: "unexpected"');
  });

  it("includes the value in the error message", () => {
    const value = { foo: "bar" } as never;
    expect(() => assertNever(value)).toThrow('Unexpected value: {"foo":"bar"}');
  });

  it("ensures exhaustiveness at compile time", () => {
    // This test verifies that TypeScript enforces exhaustiveness checking
    type Status = "pending" | "success" | "error";

    function handleStatus(status: Status): string {
      switch (status) {
        case "pending":
          return "Loading...";
        case "success":
          return "Done!";
        case "error":
          return "Failed!";
        default:
          // If we add a new status to the union, TypeScript will error here
          // because status won't be assignable to never
          return assertNever(status);
      }
    }

    expect(handleStatus("pending")).toBe("Loading...");
    expect(handleStatus("success")).toBe("Done!");
    expect(handleStatus("error")).toBe("Failed!");
  });

  it("works with discriminated unions", () => {
    type Result =
      | { type: "success"; value: number }
      | { type: "error"; message: string };

    function handleResult(result: Result): string {
      switch (result.type) {
        case "success":
          return `Success: ${result.value}`;
        case "error":
          return `Error: ${result.message}`;
        default:
          return assertNever(result);
      }
    }

    expect(handleResult({ type: "success", value: 42 })).toBe("Success: 42");
    expect(handleResult({ type: "error", message: "oops" })).toBe("Error: oops");
  });
});
