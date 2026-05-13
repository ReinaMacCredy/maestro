#!/usr/bin/env bun
/**
 * Add explicit Promise return types to async functions.
 * Handles common patterns in the Maestro codebase.
 */

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

let filesProcessed = 0;
let filesModified = 0;
let functionsFixed = 0;

async function main() {
  const srcDir = join(import.meta.dir, "../src");
  await processDirectory(srcDir);
  
  console.log(`\nProcessed ${filesProcessed} files`);
  console.log(`Modified ${filesModified} files`);
  console.log(`Fixed ${functionsFixed} async functions`);
}

async function processDirectory(dir: string): Promise<void> {
  const entries = await readdir(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);

    if (entry.isDirectory()) {
      await processDirectory(fullPath);
    } else if (entry.isFile() && entry.name.endsWith(".ts") && !entry.name.endsWith(".test.ts")) {
      await processFile(fullPath);
    }
  }
}

async function processFile(filePath: string): Promise<void> {
  filesProcessed++;
  const content = await readFile(filePath, "utf-8");
  const lines = content.split("\n");
  let modified = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    
    // Skip if already has Promise return type
    if (line.includes("): Promise<")) {
      continue;
    }

    // Skip comments
    if (line.trim().startsWith("//") || line.trim().startsWith("*")) {
      continue;
    }

    // Pattern 1: async function name(...) {  (single line)
    if (/async\s+function\s+\w+\s*\([^)]*\)\s*\{/.test(line)) {
      lines[i] = line.replace(/(\([^)]*\))(\s*)\{/, "$1: Promise<void>$2{");
      modified = true;
      functionsFixed++;
      continue;
    }

    // Pattern 2: async name(...) {  (methods, single line)
    if (/async\s+\w+\s*\([^)]*\)\s*\{/.test(line) && !line.includes("=>")) {
      lines[i] = line.replace(/(\([^)]*\))(\s*)\{/, "$1: Promise<void>$2{");
      modified = true;
      functionsFixed++;
      continue;
    }

    // Pattern 3: async (...) => {  (arrow functions)
    if (/async\s*\([^)]*\)\s*=>/.test(line)) {
      lines[i] = line.replace(/(\([^)]*\))(\s*)=>/, "$1: Promise<void>$2=>");
      modified = true;
      functionsFixed++;
      continue;
    }

    // Pattern 4: .action(async (opts) => {
    if (/\.action\(async\s*\([^)]*\)\s*=>/.test(line)) {
      lines[i] = line.replace(/async\s*(\([^)]*\))(\s*)=>/, "async $1: Promise<void>$2=>");
      modified = true;
      functionsFixed++;
      continue;
    }

    // Pattern 5: Multi-line signatures - async function/method ending with )
    // Check if next line starts with { and current line ends with )
    if (i < lines.length - 1) {
      const nextLine = lines[i + 1];
      
      // async function name(...) followed by { on next line
      if (/async\s+function\s+\w+\s*\([^)]*\)\s*$/.test(line) && nextLine.trim().startsWith("{")) {
        lines[i] = line + ": Promise<void>";
        modified = true;
        functionsFixed++;
        continue;
      }

      // async name(...) followed by { on next line (methods)
      if (/async\s+\w+\s*\([^)]*\)\s*$/.test(line) && nextLine.trim().startsWith("{") && !line.includes("=>")) {
        lines[i] = line + ": Promise<void>";
        modified = true;
        functionsFixed++;
        continue;
      }

      // Multi-line function signature ending with )
      if (/\)\s*$/.test(line) && nextLine.trim().startsWith("{")) {
        // Look back to find if this is part of an async function
        for (let j = i; j >= Math.max(0, i - 10); j--) {
          if (/async\s+(function\s+)?\w+\s*\(/.test(lines[j])) {
            lines[i] = line + ": Promise<void>";
            modified = true;
            functionsFixed++;
            break;
          }
        }
      }
    }
  }

  if (modified) {
    await writeFile(filePath, lines.join("\n"), "utf-8");
    filesModified++;
  }
}

main().catch(console.error);
