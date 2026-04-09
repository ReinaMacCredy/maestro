import { describe, expect, it } from "bun:test";
import {
  parseRawOutput,
  parseStreamJsonOutput,
} from "@/features/worker";

describe("stream-json parser", () => {
  it("extracts text from line-delimited json", () => {
    const raw = [
      JSON.stringify({ type: "message", text: "hello" }),
      JSON.stringify({ type: "result", result: "world" }),
    ].join("\n");

    expect(parseStreamJsonOutput(raw, "claude-code")).toBe("hello\nworld");
  });

  it("falls back gracefully for malformed json", () => {
    const raw = "{bad json}\n";
    expect(parseStreamJsonOutput(raw, "gemini")).toBe("{bad json}");
  });

  it("returns trimmed raw output", () => {
    expect(parseRawOutput("  hi there \n")).toBe("hi there");
  });
});
