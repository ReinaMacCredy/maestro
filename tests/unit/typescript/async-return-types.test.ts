import { describe, it, expect } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

/**
 * Test to ensure all async functions have explicit Promise return types.
 * This enforces the TypeScript safety requirement from milestone m0.
 */
describe("Async function return types", () => {
  it("should have explicit Promise return types for all async functions", async () => {
    const srcDir = join(import.meta.dir, "../../../src");
    const violations: Array<{ file: string; line: number; snippet: string }> = [];

    await scanDirectory(srcDir, violations);

    if (violations.length > 0) {
      const message = [
        `Found ${violations.length} async functions without explicit Promise return types:`,
        ...violations.slice(0, 20).map((v) => `  ${v.file}:${v.line} - ${v.snippet}`),
        violations.length > 20 ? `  ... and ${violations.length - 20} more` : "",
      ]
        .filter(Boolean)
        .join("\n");

      console.log(message);
      expect(violations.length).toBe(0);
    }
  });
});

async function scanDirectory(
  dir: string,
  violations: Array<{ file: string; line: number; snippet: string }>
): Promise<void> {
  const entries = await readdir(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);

    if (entry.isDirectory()) {
      await scanDirectory(fullPath, violations);
    } else if (entry.isFile() && entry.name.endsWith(".ts") && !entry.name.endsWith(".test.ts")) {
      await scanFile(fullPath, violations);
    }
  }
}

async function scanFile(
  filePath: string,
  violations: Array<{ file: string; line: number; snippet: string }>
): Promise<void> {
  const content = await readFile(filePath, "utf-8");
  const lines = content.split("\n");

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;
    
    const lineNumber = i + 1;

    // Skip comments
    if (line.trim().startsWith("//") || line.trim().startsWith("*")) {
      continue;
    }

    // Match async function declarations
    const asyncMatch = line.match(/\basync\s+(?:function\s+)?(\w+)?\s*\(/);
    if (asyncMatch) {
      let foundPromiseReturn = false;
      
      // Look ahead to find the closing ) and check for : Promise<
      // Use 30 lines to handle deeply nested input objects
      for (let j = i; j < Math.min(i + 30, lines.length); j++) {
        const checkLine = lines[j];
        if (!checkLine) continue;
        
        // Check if this line has ): Promise< (closing paren with return type on same line)
        if (/\):\s*Promise</.test(checkLine)) {
          foundPromiseReturn = true;
          break;
        }
        
        // Check if this line has ) followed by next line starting with : Promise<
        if (checkLine.trim().endsWith("),") || checkLine.trim().endsWith(")")) {
          // Check current line for : Promise< after the )
          if (checkLine.includes(": Promise<")) {
            foundPromiseReturn = true;
            break;
          }
          // Check next line for : Promise<
          const nextLine = lines[j + 1];
          if (nextLine && /^\s*:\s*Promise</.test(nextLine)) {
            foundPromiseReturn = true;
            break;
          }
        }
      }
      
      if (!foundPromiseReturn) {
        const relativePath = filePath.replace(process.cwd() + "/", "");
        violations.push({
          file: relativePath,
          line: lineNumber,
          snippet: line.trim().slice(0, 80),
        });
      }
    }
  }
}
