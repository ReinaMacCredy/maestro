#!/usr/bin/env bun
/**
 * Script to add explicit Promise return types to async functions.
 * This is a one-time migration for milestone m0-async-return-types.
 */

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

interface Fix {
  file: string;
  line: number;
  original: string;
  fixed: string;
}

const fixes: Fix[] = [];
let filesProcessed = 0;
let functionsFixed = 0;

async function main() {
  const srcDir = join(import.meta.dir, "../src");
  await processDirectory(srcDir);
  
  console.log(`\nProcessed ${filesProcessed} files`);
  console.log(`Fixed ${functionsFixed} async functions`);
  console.log(`\nSample fixes:`);
  fixes.slice(0, 5).forEach(fix => {
    console.log(`  ${fix.file}:${fix.line}`);
    console.log(`    - ${fix.original}`);
    console.log(`    + ${fix.fixed}`);
  });
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
  const newLines: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    
    // Skip if already has Promise return type
    if (line.includes("): Promise<")) {
      newLines.push(line);
      continue;
    }

    // Match async function patterns
    const asyncMatch = line.match(/^(\s*)(export\s+)?(async\s+function\s+\w+\s*\([^)]*\))(\s*\{)?$/);
    if (asyncMatch) {
      const [, indent, exportKeyword, funcSignature, brace] = asyncMatch;
      const fixed = `${indent}${exportKeyword || ""}${funcSignature}: Promise<void>${brace || ""}`;
      fixes.push({
        file: filePath.replace(process.cwd() + "/", ""),
        line: i + 1,
        original: line.trim(),
        fixed: fixed.trim(),
      });
      newLines.push(fixed);
      functionsFixed++;
      modified = true;
      continue;
    }

    // Match async method patterns
    const methodMatch = line.match(/^(\s*)(private\s+|public\s+|protected\s+)?(async\s+\w+\s*\([^)]*\))(\s*\{)?$/);
    if (methodMatch) {
      const [, indent, visibility, methodSignature, brace] = methodMatch;
      const fixed = `${indent}${visibility || ""}${methodSignature}: Promise<void>${brace || ""}`;
      fixes.push({
        file: filePath.replace(process.cwd() + "/", ""),
        line: i + 1,
        original: line.trim(),
        fixed: fixed.trim(),
      });
      newLines.push(fixed);
      functionsFixed++;
      modified = true;
      continue;
    }

    // Match async arrow function in object literal
    const arrowMatch = line.match(/^(\s*)(\w+):\s*async\s*\(([^)]*)\)\s*=>\s*\{?$/);
    if (arrowMatch) {
      const [, indent, name, params] = arrowMatch;
      const fixed = `${indent}${name}: async (${params}): Promise<void> => {`;
      fixes.push({
        file: filePath.replace(process.cwd() + "/", ""),
        line: i + 1,
        original: line.trim(),
        fixed: fixed.trim(),
      });
      newLines.push(fixed);
      functionsFixed++;
      modified = true;
      continue;
    }

    newLines.push(line);
  }

  if (modified) {
    await writeFile(filePath, newLines.join("\n"), "utf-8");
  }
}

main().catch(console.error);
