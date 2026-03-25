/**
 * Dynamic workflow tool registry.
 * Tools self-declare their stage, category, and prerequisites at registration.
 * Replaces the static PLAYBOOKS lookup table.
 */

import type { ToolboxRegistry } from '../../toolbox/registry.ts';

export type ToolCategory = 'primary' | 'conditional' | 'meta' | 'utility';

export interface ToolWorkflowMeta {
  /** Pipeline stages where this tool is relevant. */
  stages: string[];
  /** Tool category for filtering. */
  category: ToolCategory;
  /** External tool name required (for conditional tools). */
  requires?: string;
  /** Other tool names that should be called before this one. */
  prerequisites?: string[];
  /** Context-aware hint displayed when this tool is recommended. */
  contextHint?: string;
}

export interface RegisteredTool {
  name: string;
  meta: ToolWorkflowMeta;
}

export class WorkflowRegistry {
  private tools: Map<string, ToolWorkflowMeta> = new Map();

  /** Register a tool with its workflow metadata. */
  register(toolName: string, meta: ToolWorkflowMeta): void {
    this.tools.set(toolName, meta);
  }

  /**
   * Get tools for a pipeline stage, optionally filtered by toolbox availability.
   * Returns only PRIMARY and CONDITIONAL tools (meta/utility excluded from recommendations).
   */
  getToolsForStage(stage: string, toolbox?: ToolboxRegistry): string[] {
    const result: string[] = [];

    for (const [name, meta] of this.tools) {
      if (!meta.stages.includes(stage)) continue;
      if (meta.category !== 'primary' && meta.category !== 'conditional') continue;

      // Filter conditional tools by toolbox availability
      if (meta.category === 'conditional' && meta.requires) {
        if (!toolbox || !toolbox.isAvailable(meta.requires)) continue;
      }

      result.push(name);
    }

    return result;
  }

  /** Get all registered tools with their metadata. */
  getAll(): RegisteredTool[] {
    return [...this.tools.entries()].map(([name, meta]) => ({ name, meta }));
  }

  /** Get category for a specific tool. */
  getCategory(toolName: string): ToolCategory | null {
    return this.tools.get(toolName)?.category ?? null;
  }

  /** Get metadata for a specific tool. */
  getMeta(toolName: string): ToolWorkflowMeta | null {
    return this.tools.get(toolName) ?? null;
  }

  /** Total number of registered tools. */
  get size(): number {
    return this.tools.size;
  }
}
