import { describe, expect, it } from "bun:test";
import {
  fail,
  fromMaestroError,
  ok,
  toCallToolResult,
} from "@/features/mcp/server/errors.js";
import { MaestroError } from "@/shared/errors.js";

describe("ok / fail", () => {
  it("ok wraps a payload as a success result", () => {
    const r = ok({ x: 1 });
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.data.x).toBe(1);
    }
  });

  it("fail wraps code, message, and hints", () => {
    const r = fail("CODE_X", "boom", ["hint1"]);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.code).toBe("CODE_X");
      expect(r.error.message).toBe("boom");
      expect(r.error.hints).toEqual(["hint1"]);
    }
  });

  it("fail defaults hints to []", () => {
    const r = fail("CODE", "msg");
    if (!r.ok) {
      expect(r.error.hints).toEqual([]);
    }
  });
});

describe("fromMaestroError", () => {
  it("extracts hints from a MaestroError", () => {
    const err = new MaestroError("contract not found", ["create one"]);
    const r = fromMaestroError(err);
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.message).toBe("contract not found");
      expect(r.error.hints).toEqual(["create one"]);
    }
  });

  it("derives NOT_FOUND from MaestroError messages mentioning 'not found'", () => {
    const r = fromMaestroError(new MaestroError("Task not found", []));
    if (!r.ok) expect(r.error.code).toBe("NOT_FOUND");
  });

  it("derives CYCLE_DETECTED from cycle messages", () => {
    const r = fromMaestroError(new MaestroError("blocker cycle detected", []));
    if (!r.ok) expect(r.error.code).toBe("CYCLE_DETECTED");
  });

  it("derives OWNERSHIP_CONFLICT from ownership messages", () => {
    const r = fromMaestroError(new MaestroError("task owned by another session", []));
    if (!r.ok) expect(r.error.code).toBe("OWNERSHIP_CONFLICT");
  });

  it("derives CONTRACT_ERROR from generic contract messages", () => {
    const r = fromMaestroError(new MaestroError("contract amendment budget exhausted", []));
    if (!r.ok) expect(r.error.code).toBe("CONTRACT_ERROR");
  });

  it("falls back to the provided fallback code on opaque MaestroErrors", () => {
    const r = fromMaestroError(new MaestroError("something opaque", []), "FALLBACK");
    if (!r.ok) expect(r.error.code).toBe("FALLBACK");
  });

  it("wraps a plain Error with the fallback code and empty hints", () => {
    const r = fromMaestroError(new Error("plain"), "FALLBACK");
    if (!r.ok) {
      expect(r.error.code).toBe("FALLBACK");
      expect(r.error.message).toBe("plain");
      expect(r.error.hints).toEqual([]);
    }
  });

  it("stringifies non-Error throwables", () => {
    const r = fromMaestroError("oops", "FALLBACK");
    if (!r.ok) {
      expect(r.error.code).toBe("FALLBACK");
      expect(r.error.message).toBe("oops");
    }
  });
});

describe("toCallToolResult", () => {
  it("renders success content as JSON text and structured payload", () => {
    const out = toCallToolResult(ok({ a: 1 }));
    expect(out.isError).toBeUndefined();
    expect(out.content[0].type).toBe("text");
    expect(JSON.parse(out.content[0].text)).toEqual({ a: 1 });
    expect(out.structuredContent).toEqual({ a: 1 });
  });

  it("renders failure with isError=true and code/message/hints in payload", () => {
    const out = toCallToolResult(fail("CODE", "msg", ["hint"]));
    expect(out.isError).toBe(true);
    const parsed = JSON.parse(out.content[0].text);
    expect(parsed.code).toBe("CODE");
    expect(parsed.message).toBe("msg");
    expect(parsed.hints).toEqual(["hint"]);
    expect(out.structuredContent).toEqual({ code: "CODE", message: "msg", hints: ["hint"] });
  });
});
