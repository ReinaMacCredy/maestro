import { describe, expect, it } from "bun:test";
import {
  DEFAULT_PAGE_SIZE,
  MAX_PAGE_SIZE,
  paginate,
} from "@/features/mcp/server/pagination.js";

describe("paginate", () => {
  const items = Array.from({ length: 50 }, (_, i) => i);

  it("returns the default page when limit and offset are undefined", () => {
    const page = paginate(items, undefined, undefined);
    expect(page.items).toHaveLength(DEFAULT_PAGE_SIZE);
    expect(page.items[0]).toBe(0);
    expect(page.pagination.total).toBe(50);
    expect(page.pagination.limit).toBe(DEFAULT_PAGE_SIZE);
    expect(page.pagination.offset).toBe(0);
    expect(page.pagination.hasMore).toBe(true);
  });

  it("clamps limit to MAX_PAGE_SIZE", () => {
    const page = paginate(items, 1000, 0);
    expect(page.pagination.limit).toBe(MAX_PAGE_SIZE);
  });

  it("clamps limit to at least 1", () => {
    const page = paginate(items, 0, 0);
    expect(page.pagination.limit).toBe(1);
    expect(page.items).toHaveLength(1);
  });

  it("clamps negative offset to 0", () => {
    const page = paginate(items, 5, -10);
    expect(page.pagination.offset).toBe(0);
    expect(page.items[0]).toBe(0);
  });

  it("returns hasMore=false when the page covers the tail", () => {
    const page = paginate(items, 10, 45);
    expect(page.items).toHaveLength(5);
    expect(page.pagination.hasMore).toBe(false);
  });

  it("returns hasMore=false on an empty list", () => {
    const page = paginate([], undefined, undefined);
    expect(page.items).toHaveLength(0);
    expect(page.pagination.total).toBe(0);
    expect(page.pagination.hasMore).toBe(false);
  });

  it("returns an empty slice when offset is past the end", () => {
    const page = paginate(items, 10, 200);
    expect(page.items).toHaveLength(0);
    expect(page.pagination.hasMore).toBe(false);
  });
});
