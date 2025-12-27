#!/usr/bin/env node
/**
 * Continuity Hooks for Claude Code
 * 
 * Provides automatic session state preservation across sessions and compactions.
 * 
 * Usage: node continuity.js <hook-event>
 *   SessionStart - Load LEDGER.md + last handoff
 *   PreCompact   - Create handoff before compaction
 *   PostToolUse  - Track modified files
 *   Stop         - Archive session on exit
 */

import * as fs from "fs";
import * as path from "path";

const VERSION = "1.0.0";
const STALE_THRESHOLD_HOURS = 24;

let directoriesEnsured = false;

interface HookOutput {
  hookSpecificOutput?: {
    hookEventName: string;
    additionalContext?: string;
  };
}

function findConductorRoot(): string | null {
  let dir = process.cwd();
  const root = path.parse(dir).root;
  
  while (dir !== root) {
    if (fs.existsSync(path.join(dir, "conductor"))) {
      return path.join(dir, "conductor");
    }
    dir = path.dirname(dir);
  }
  return null;
}

function ensureDirectories(conductorRoot: string): void {
  if (directoriesEnsured) return;
  
  const dirs = [
    path.join(conductorRoot, "sessions", "active"),
    path.join(conductorRoot, "sessions", "archive"),
    path.join(conductorRoot, ".cache"),
  ];
  
  for (const dir of dirs) {
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
  }
  
  directoriesEnsured = true;
}

function isStale(mtime: Date): boolean {
  const now = new Date();
  const diffHours = (now.getTime() - mtime.getTime()) / (1000 * 60 * 60);
  return diffHours > STALE_THRESHOLD_HOURS;
}

function formatTimestamp(): string {
  return new Date().toISOString().replace("T", "-").replace(/[:.]/g, "-").slice(0, 16);
}

function getLatestHandoff(archiveDir: string): string | null {
  if (!fs.existsSync(archiveDir)) return null;
  
  const files = fs.readdirSync(archiveDir)
    .filter(f => f.endsWith(".md"))
    .sort()
    .reverse();
  
  if (files.length === 0) return null;
  
  return fs.readFileSync(path.join(archiveDir, files[0]), "utf-8");
}

function createHandoff(
  conductorRoot: string,
  ledgerPath: string,
  trigger: "manual" | "pre-compact" | "session-end" | "stale"
): void {
  const archiveDir = path.join(conductorRoot, "sessions", "archive");
  ensureDirectories(conductorRoot);
  
  const timestamp = formatTimestamp();
  const handoffName = `${timestamp}-${trigger}.md`;
  const handoffPath = path.join(archiveDir, handoffName);
  
  let content: string;
  if (fs.existsSync(ledgerPath)) {
    const ledgerContent = fs.readFileSync(ledgerPath, "utf-8");
    content = `---
date: ${new Date().toISOString()}
session_id: ${process.env.CLAUDE_SESSION_ID || "unknown"}
trigger: ${trigger}
status: ${trigger === "session-end" ? "complete" : "handoff"}
---

# Session Handoff

## Source

Archived from LEDGER.md on ${trigger}.

${ledgerContent}
`;
  } else {
    content = `---
date: ${new Date().toISOString()}
session_id: ${process.env.CLAUDE_SESSION_ID || "unknown"}
trigger: ${trigger}
status: handoff
---

# Session Handoff

## Summary

No active ledger at time of handoff.
`;
  }
  
  fs.writeFileSync(handoffPath, content, "utf-8");
  console.error(`[continuity] Created handoff: ${handoffName}`);
}

function handleSessionStart(): void {
  const conductorRoot = findConductorRoot();
  if (!conductorRoot) {
    const output: HookOutput = {
      hookSpecificOutput: {
        hookEventName: "SessionStart",
        additionalContext: "[continuity] No conductor/ directory found. Session continuity disabled.",
      },
    };
    console.log(JSON.stringify(output));
    return;
  }
  
  ensureDirectories(conductorRoot);
  
  const ledgerPath = path.join(conductorRoot, "sessions", "active", "LEDGER.md");
  const archiveDir = path.join(conductorRoot, "sessions", "archive");
  
  let contextParts: string[] = [];
  
  if (fs.existsSync(ledgerPath)) {
    const stats = fs.statSync(ledgerPath);
    
    if (isStale(stats.mtime)) {
      createHandoff(conductorRoot, ledgerPath, "stale");
      fs.unlinkSync(ledgerPath);
      contextParts.push("[continuity] Previous session was stale (>24h). Archived and starting fresh.");
    } else {
      const ledgerContent = fs.readFileSync(ledgerPath, "utf-8");
      contextParts.push(`<ledger>\n${ledgerContent}\n</ledger>`);
    }
  }
  
  const lastHandoff = getLatestHandoff(archiveDir);
  if (lastHandoff) {
    contextParts.push(`<last_handoff>\n${lastHandoff}\n</last_handoff>`);
  }
  
  if (contextParts.length === 0) {
    contextParts.push("[continuity] No existing session state. Starting fresh.");
  }
  
  const output: HookOutput = {
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext: `<continuity_context>\n${contextParts.join("\n\n")}\n</continuity_context>`,
    },
  };
  
  console.log(JSON.stringify(output));
}

function handlePreCompact(): void {
  const conductorRoot = findConductorRoot();
  if (!conductorRoot) {
    console.error("[continuity] No conductor/ directory found. Skipping pre-compact handoff.");
    return;
  }
  
  const ledgerPath = path.join(conductorRoot, "sessions", "active", "LEDGER.md");
  createHandoff(conductorRoot, ledgerPath, "pre-compact");
}

function handlePostToolUse(): void {
  const conductorRoot = findConductorRoot();
  if (!conductorRoot) return;
  
  const ledgerPath = path.join(conductorRoot, "sessions", "active", "LEDGER.md");
  
  const toolInput = process.env.CLAUDE_TOOL_INPUT;
  if (!toolInput) return;
  
  let filePath: string | undefined;
  try {
    const parsed = JSON.parse(toolInput);
    filePath = parsed.file_path || parsed.path || parsed.filePath;
  } catch {
    return;
  }
  
  if (!filePath) return;
  
  ensureDirectories(conductorRoot);
  
  let content: string;
  if (fs.existsSync(ledgerPath)) {
    content = fs.readFileSync(ledgerPath, "utf-8");
    
    if (content.includes("**Modified:**")) {
      if (!content.includes(filePath)) {
        content = content.replace(
          "**Modified:**",
          `**Modified:**\n- \`${filePath}\``
        );
      }
    } else {
      content += `\n\n## Working Set\n\n**Modified:**\n- \`${filePath}\`\n`;
    }
    
    content = content.replace(
      /updated: .+/,
      `updated: ${new Date().toISOString()}`
    );
  } else {
    content = `---
updated: ${new Date().toISOString()}
session_id: ${process.env.CLAUDE_SESSION_ID || "unknown"}
platform: claude
---

# Session Ledger

## Working Set

**Modified:**
- \`${filePath}\`
`;
  }
  
  fs.writeFileSync(ledgerPath, content, "utf-8");
}

function handleStop(): void {
  const conductorRoot = findConductorRoot();
  if (!conductorRoot) return;
  
  const ledgerPath = path.join(conductorRoot, "sessions", "active", "LEDGER.md");
  if (fs.existsSync(ledgerPath)) {
    createHandoff(conductorRoot, ledgerPath, "session-end");
  }
}

function main(): void {
  const args = process.argv.slice(2);
  
  if (args.includes("--version") || args.includes("-v")) {
    console.log(`continuity-hooks v${VERSION}`);
    process.exit(0);
  }
  
  const hookEvent = args[0];
  
  try {
    switch (hookEvent) {
      case "SessionStart":
        handleSessionStart();
        break;
      case "PreCompact":
        handlePreCompact();
        break;
      case "PostToolUse":
        handlePostToolUse();
        break;
      case "Stop":
        handleStop();
        break;
      default:
        console.error(`[continuity] Unknown hook event: ${hookEvent}`);
        console.error("Usage: continuity.js <SessionStart|PreCompact|PostToolUse|Stop>");
        process.exit(1);
    }
  } catch (error) {
    console.error(`[continuity] Error in ${hookEvent}:`, error);
  }
  
  process.exit(0);
}

main();
