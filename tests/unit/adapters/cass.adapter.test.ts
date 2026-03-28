import { describe, expect, it } from "bun:test";
import { ShellCassAdapter } from "../../../src/adapters/cass.adapter.js";

const cass = new ShellCassAdapter();

describe("ShellCassAdapter", () => {
  describe("isAvailable", () => {
    it("returns true when cass is installed", async () => {
      const result = await cass.isAvailable();
      // CASS is a required dependency, should be available
      expect(result).toBe(true);
    });

    it("returns false when cass binary not found", async () => {
      const badCass = new ShellCassAdapter("/nonexistent/cass");
      const result = await badCass.isAvailable();
      expect(result).toBe(false);
    });
  });

  describe("search", () => {
    it("returns empty results for nonsense query", async () => {
      const result = await cass.search("zzznonexistentqueryzz", {
        limit: 1,
      });
      expect(result.query).toBe("zzznonexistentqueryzz");
      expect(Array.isArray(result.hits)).toBe(true);
    });

    it("returns structured results with correct fields", async () => {
      const result = await cass.search("test", { limit: 3 });
      expect(result).toHaveProperty("query");
      expect(result).toHaveProperty("count");
      expect(result).toHaveProperty("totalMatches");
      expect(result).toHaveProperty("hits");
    });
  });
});
