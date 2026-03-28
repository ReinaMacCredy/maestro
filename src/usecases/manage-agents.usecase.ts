import { existsSync } from "node:fs";
import {
  SUPPORTED_AGENTS,
  agentConfigPath,
  agentConfigDirPath,
  type AgentConfigSpec,
} from "../domain/agents.js";
import { AGENT_INSTRUCTION_BLOCK } from "../domain/defaults.js";
import { renderTemplate } from "../lib/template.js";
import { readText, writeText, ensureDir } from "../lib/fs.js";
import {
  hasBlock,
  extractBlock,
  injectBlock,
  replaceBlock,
  removeBlock,
  removeLegacyBlock,
  wrapBlock,
} from "../lib/agent-block.js";
import { dirname } from "node:path";

export interface InjectResult {
  readonly agent: string;
  readonly action: "injected" | "updated" | "migrated" | "skipped" | "not-detected";
  readonly configPath: string;
}

export interface RemoveResult {
  readonly agent: string;
  readonly action: "removed" | "not-found" | "not-detected";
  readonly configPath: string;
}

function renderBlock(agent: AgentConfigSpec): string {
  return renderTemplate(AGENT_INSTRUCTION_BLOCK, { agent: agent.agentFlag });
}

export async function injectAgentBlocks(): Promise<InjectResult[]> {
  const results: InjectResult[] = [];

  for (const agent of SUPPORTED_AGENTS) {
    const configPath = agentConfigPath(agent);
    const dirPath = agentConfigDirPath(agent);

    if (!existsSync(dirPath)) {
      results.push({ agent: agent.displayName, action: "not-detected", configPath });
      continue;
    }

    const rendered = renderBlock(agent);
    const existing = await readText(configPath) ?? "";

    // Check for legacy unmarked block -- migrate it
    if (!hasBlock(existing)) {
      const cleaned = removeLegacyBlock(existing);
      if (cleaned !== null) {
        const injected = injectBlock(cleaned, rendered);
        await ensureDir(dirname(configPath));
        await writeText(configPath, injected);
        results.push({ agent: agent.displayName, action: "migrated", configPath });
        continue;
      }
    }

    if (hasBlock(existing)) {
      const currentBlock = extractBlock(existing);
      if (currentBlock === rendered) {
        results.push({ agent: agent.displayName, action: "skipped", configPath });
      } else {
        const updated = replaceBlock(existing, rendered)!;
        await writeText(configPath, updated);
        results.push({ agent: agent.displayName, action: "updated", configPath });
      }
    } else {
      const injected = injectBlock(existing, rendered);
      await ensureDir(dirname(configPath));
      await writeText(configPath, injected);
      results.push({ agent: agent.displayName, action: "injected", configPath });
    }
  }

  return results;
}

export async function removeAgentBlocks(): Promise<RemoveResult[]> {
  const results: RemoveResult[] = [];

  for (const agent of SUPPORTED_AGENTS) {
    const configPath = agentConfigPath(agent);
    const dirPath = agentConfigDirPath(agent);

    if (!existsSync(dirPath)) {
      results.push({ agent: agent.displayName, action: "not-detected", configPath });
      continue;
    }

    const existing = await readText(configPath);
    if (!existing || !hasBlock(existing)) {
      results.push({ agent: agent.displayName, action: "not-found", configPath });
      continue;
    }

    const cleaned = removeBlock(existing)!;
    await writeText(configPath, cleaned);
    results.push({ agent: agent.displayName, action: "removed", configPath });
  }

  return results;
}
