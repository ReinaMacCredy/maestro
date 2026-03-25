/**
 * Detect available external research tools by checking .mcp.json files.
 */

import { readJson } from '../../core/fs-io.ts';
import * as path from 'path';
import { homedir } from 'os';

const KNOWN_TOOLS = ['context7', 'notebooklm'] as const;
type ResearchTool = typeof KNOWN_TOOLS[number];

interface McpConfig {
  mcpServers?: Record<string, unknown>;
}

export function detectResearchTools(projectDir: string): ResearchTool[] {
  const detected: ResearchTool[] = [];

  // Check project-level .mcp.json
  const projectMcp = readJson<McpConfig>(path.join(projectDir, '.mcp.json'));
  // Check user-level ~/.claude/mcp.json
  const userMcp = readJson<McpConfig>(path.join(homedir(), '.claude', 'mcp.json'));

  const allServers = {
    ...userMcp?.mcpServers,
    ...projectMcp?.mcpServers,
  };

  for (const tool of KNOWN_TOOLS) {
    // Check for exact name or partial match in server keys
    const hasMatch = Object.keys(allServers).some(key =>
      key.toLowerCase().includes(tool)
    );
    if (hasMatch) detected.push(tool);
  }

  return detected;
}
