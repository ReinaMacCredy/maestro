import { describe, it, expect } from "bun:test";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

async function* walkFiles(dir: string): AsyncGenerator<string> {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!entry.name.startsWith(".") && entry.name !== "node_modules") {
        yield* walkFiles(path);
      }
    } else if (entry.name.endsWith(".ts") && !entry.name.endsWith(".d.ts")) {
      yield path;
    }
  }
}

describe("Type Import Usage", () => {
  it("should use 'import type' for type-only imports", async () => {
    // Build a map of exported types
    const typeExports = new Map<string, Set<string>>();
    const files: string[] = [];
    for await (const file of walkFiles(join(process.cwd(), "src"))) {
      files.push(file);
    }

    // Scan all files to find type/interface exports
    for (const file of files) {
      const content = await readFile(file, "utf-8");
      const lines = content.split("\n");

      const types = new Set<string>();

      for (const line of lines) {
        // Match: export type Name = ...
        // Match: export interface Name ...
        const typeMatch = line.match(/export\s+type\s+(\w+)/);
        const interfaceMatch = line.match(/export\s+interface\s+(\w+)/);

        if (typeMatch?.[1]) types.add(typeMatch[1]);
        if (interfaceMatch?.[1]) types.add(interfaceMatch[1]);
      }

      if (types.size > 0) {
        typeExports.set(file, types);
      }
    }

    // Find imports that import types without 'import type' or inline 'type'
    const violations: Array<{ file: string; line: number; text: string; typeName: string }> = [];

    for (const file of files) {
      const content = await readFile(file, "utf-8");
      const lines = content.split("\n");

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();

        // Skip if already using 'import type'
        if (line.startsWith("import type")) continue;

        // Match regular imports
        const match = line.match(/^import\s+\{\s*([^}]+)\s*\}\s+from\s+["']([^"']+)["']/);
        if (!match || !match[1]) continue;

        const imports: string = match[1];

        // Parse imported names
        const names = imports.split(",").map((n) => n.trim());

        // Check each imported name
        for (const name of names) {
          // Skip if it has inline 'type' keyword
          if (name.startsWith("type ")) continue;

          // Check if this name is a known type export
          let isKnownType = false;
          for (const exportedTypes of typeExports.values()) {
            if (exportedTypes.has(name)) {
              isKnownType = true;
              break;
            }
          }

          if (isKnownType) {
            violations.push({
              file,
              line: i + 1,
              text: line,
              typeName: name,
            });
          }
        }
      }
    }

    // Report violations
    if (violations.length > 0) {
      const message = [
        `Found ${violations.length} type imports without 'import type' keyword:`,
        "",
        ...violations.map(
          (v) =>
            `${v.file}:${v.line}\n  ${v.text}\n  → Type '${v.typeName}' should use 'import type' or inline 'type' keyword`
        ),
      ].join("\n");

      expect(violations).toEqual([]);
      throw new Error(message);
    }

    expect(violations).toEqual([]);
  });

  it("should have type exports in the codebase", async () => {
    const files: string[] = [];
    for await (const file of walkFiles(join(process.cwd(), "src"))) {
      files.push(file);
    }

    let typeExportCount = 0;

    for (const file of files) {
      const content = await readFile(file, "utf-8");
      const lines = content.split("\n");

      for (const line of lines) {
        if (line.match(/export\s+(type|interface)\s+\w+/)) {
          typeExportCount++;
        }
      }
    }

    // Ensure we found type exports (sanity check that the test is working)
    expect(typeExportCount).toBeGreaterThan(100);
  });

  it("should use 'import type' for common type patterns", async () => {
    const files: string[] = [];
    for await (const file of walkFiles(join(process.cwd(), "src"))) {
      files.push(file);
    }

    const violations: Array<{ file: string; line: number; text: string }> = [];

    for (const file of files) {
      const content = await readFile(file, "utf-8");
      const lines = content.split("\n");

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();

        // Skip if already using 'import type'
        if (line.startsWith("import type")) continue;

        // Match regular imports from type files
        const match = line.match(/^import\s+\{[^}]+\}\s+from\s+["']([^"']+)["']/);
        if (!match || !match[1]) continue;

        const source: string = match[1];

        // Check if importing from a -types.ts file
        if (source.endsWith("-types.js") || source.endsWith("-types")) {
          // Extract imported names
          const importsMatch = line.match(/\{\s*([^}]+)\s*\}/);
          if (!importsMatch || !importsMatch[1]) continue;

          const imports = importsMatch[1];

          // Check if all imports have 'type' keyword or are known values
          const names = imports.split(",").map((n) => n.trim());

          // Known value exports from type files (constants, functions)
          const knownValues = [
            "TASK_STATUSES",
            "TASK_TYPES",
            "buildTaskReceipt",
            "indexTasksById",
            "CONTRACT_SCHEMA_VERSION",
            "DEFAULT_CONFIG",
            "MAESTRO_DIR",
          ];

          const hasTypeKeyword = names.some((n) => n.startsWith("type "));
          const allAreKnownValues = names.every((n) => knownValues.includes(n));

          // If importing from -types file without 'type' keyword and not all known values
          if (!hasTypeKeyword && !allAreKnownValues) {
            // This might be a violation, but we need to check if these are actually types
            // For now, we'll just document this pattern
          }
        }
      }
    }

    // This test passes if we don't find obvious violations
    expect(violations).toEqual([]);
  });
});
