#!/usr/bin/env bun
/**
 * Find imports that are only used as types and should use 'import type' syntax
 */

import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

interface ImportInfo {
  file: string;
  line: number;
  importedNames: string[];
  source: string;
  isTypeOnly: boolean;
}

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

async function analyzeFile(filePath: string): Promise<ImportInfo[]> {
  const content = await readFile(filePath, "utf-8");
  const lines = content.split("\n");
  const imports: ImportInfo[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    
    // Skip if already using 'import type'
    if (line.trim().startsWith("import type")) {
      continue;
    }

    // Match: import { Name1, Name2 } from "source"
    const match = line.match(/^import\s+\{\s*([^}]+)\s*\}\s+from\s+["']([^"']+)["']/);
    if (!match) continue;

    const [, namesStr, source] = match;
    const importedNames = namesStr
      .split(",")
      .map((n) => n.trim().replace(/^type\s+/, ""))
      .filter(Boolean);

    // Check if all imported names are only used as types
    const usedAsValues = new Set<string>();
    const usedAsTypes = new Set<string>();

    for (const name of importedNames) {
      // Look for usage patterns in the rest of the file
      const restOfFile = lines.slice(i + 1).join("\n");
      
      // Value usage patterns:
      // - Function calls: name(...)
      // - Object access: name.something
      // - Variable assignment: const x = name
      // - Array/object literals: [name], {name}
      // - JSX: <name>
      const valuePatterns = [
        new RegExp(`\\b${name}\\s*\\(`),           // Function call
        new RegExp(`\\b${name}\\.`),               // Property access
        new RegExp(`=\\s*${name}\\b`),             // Assignment
        new RegExp(`\\[\\s*${name}\\s*[,\\]]`),    // Array literal
        new RegExp(`\\{\\s*${name}\\s*[,}]`),      // Object literal
        new RegExp(`<${name}[\\s>]`),              // JSX
        new RegExp(`extends\\s+${name}\\b`),       // Class extends
        new RegExp(`implements\\s+${name}\\b`),    // Class implements
      ];

      // Type usage patterns:
      // - Type annotations: : name
      // - Generic parameters: <name>
      // - Type assertions: as name
      // - typeof: typeof name
      const typePatterns = [
        new RegExp(`:\\s*${name}\\b`),             // Type annotation
        new RegExp(`<${name}[,>]`),                // Generic parameter
        new RegExp(`as\\s+${name}\\b`),            // Type assertion
        new RegExp(`typeof\\s+${name}\\b`),        // typeof
      ];

      let hasValueUsage = false;
      let hasTypeUsage = false;

      for (const pattern of valuePatterns) {
        if (pattern.test(restOfFile)) {
          hasValueUsage = true;
          break;
        }
      }

      for (const pattern of typePatterns) {
        if (pattern.test(restOfFile)) {
          hasTypeUsage = true;
          break;
        }
      }

      if (hasValueUsage) {
        usedAsValues.add(name);
      }
      if (hasTypeUsage) {
        usedAsTypes.add(name);
      }
    }

    // If all names are only used as types, mark as type-only
    const isTypeOnly = importedNames.every(
      (name) => usedAsTypes.has(name) && !usedAsValues.has(name)
    );

    if (isTypeOnly && importedNames.length > 0) {
      imports.push({
        file: filePath,
        line: i + 1,
        importedNames,
        source,
        isTypeOnly: true,
      });
    }
  }

  return imports;
}

async function main() {
  const srcDir = join(process.cwd(), "src");
  const typeOnlyImports: ImportInfo[] = [];

  for await (const file of walkFiles(srcDir)) {
    const imports = await analyzeFile(file);
    typeOnlyImports.push(...imports);
  }

  if (typeOnlyImports.length === 0) {
    console.log("✓ All type-only imports already use 'import type' syntax");
    process.exit(0);
  }

  console.log(`Found ${typeOnlyImports.length} imports that should use 'import type':\n`);
  
  for (const imp of typeOnlyImports) {
    const relPath = imp.file.replace(process.cwd() + "/", "");
    console.log(`${relPath}:${imp.line}`);
    console.log(`  import { ${imp.importedNames.join(", ")} } from "${imp.source}"`);
    console.log(`  → import type { ${imp.importedNames.join(", ")} } from "${imp.source}"\n`);
  }

  process.exit(1);
}

main();
