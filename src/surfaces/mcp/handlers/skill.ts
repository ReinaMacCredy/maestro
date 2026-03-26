import { z } from 'zod';
import * as fs from 'node:fs';
import * as path from 'node:path';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { ServicesThunk } from '../services-thunk.ts';
import { respond, textResponse, errorResponse, withErrorHandling } from '../respond.ts';
import { ANNOTATIONS_MUTATING } from '../annotations.ts';
import { loadSkill, loadSkillReference, listSkills } from '../../../app/skills/registry.ts';
import { MaestroError } from '../../../domain/errors.ts';
import { installSkill } from '../../../app/skills/install.ts';
import { createSkill } from '../../../app/skills/create.ts';
import { syncSkills } from '../../../app/skills/sync.ts';

export function registerSkillTools(server: McpServer, _thunk: ServicesThunk, directory?: string): void {
  // Single merged tool: action: load | list | install | create | remove | sync
  server.registerTool(
    'maestro_skill',
    {
      description:
        'Load a skill into context, browse available skills, or manage skill installation. ' +
        'Use when a playbook recommends loading a skill, or to discover/install skills for the current workflow stage. ' +
        'Actions: load (requires: name -- returns raw text), list, install (requires: source path), ' +
        'create (requires: name), remove (requires: name), sync (rescan registry). ' +
        'Example: maestro_skill({ action: "load", name: "maestro:design" })',
      inputSchema: {
        action: z.enum(['load', 'list', 'install', 'create', 'remove', 'sync'])
          .describe('Action to perform'),
        name: z.string().optional().describe('Skill name (required for load, create, remove; e.g. maestro:design)'),
        reference: z.string().optional().describe('Reference file path within skill (load only; e.g. steps/step-01.md)'),
        source: z.string().optional().describe('Path to skill directory with SKILL.md (required for install)'),
        stage: z.string().optional().describe('Pipeline stage for new skill (create only)'),
      },
      annotations: ANNOTATIONS_MUTATING,
    },
    withErrorHandling(async (input) => {
      switch (input.action) {
        case 'load': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: load', suggestions: ['Provide the name parameter.'] });
          const result = input.reference
            ? await loadSkillReference(input.name, input.reference, directory)
            : await loadSkill(input.name, directory);
          if ('error' in result) {
            throw new MaestroError(result.error, ['Use action: list to see available skills.']);
          }
          return textResponse(result.content);
        }
        case 'list': {
          const skills = await listSkills(directory);
          return respond({
            count: skills.length,
            skills: skills.map(s => ({
              name: s.name,
              description: s.description,
              source: s.source,
              ...(s.argumentHint ? { argumentHint: s.argumentHint } : {}),
              ...(s.stage ? { stage: s.stage } : {}),
              ...(s.audience ? { audience: s.audience } : {}),
            })),
          });
        }
        case 'install': {
          if (!input.source) return errorResponse({ terminal: false, reason: 'validation', error: 'source is required for action: install', suggestions: ['Provide the source parameter.'] });
          const projectRoot = directory ?? process.cwd();
          const result = installSkill(input.source, projectRoot);
          return respond({ installed: result.name, path: result.path });
        }
        case 'create': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: create', suggestions: ['Provide the name parameter.'] });
          const projectRoot = directory ?? process.cwd();
          const result = createSkill(input.name, projectRoot, input.stage);
          return respond({ created: result.name, path: result.path });
        }
        case 'remove': {
          if (!input.name) return errorResponse({ terminal: false, reason: 'validation', error: 'name is required for action: remove', suggestions: ['Provide the name parameter.'] });
          const projectRoot = directory ?? process.cwd();
          const slug = input.name.toLowerCase().replace(/[^a-z0-9-:]/g, '-').replace(/-+/g, '-');
          const skillDir = path.join(projectRoot, '.maestro', 'skills', slug);
          if (!fs.existsSync(skillDir)) {
            throw new MaestroError(`Skill '${input.name}' not found at ${skillDir}`);
          }
          fs.rmSync(skillDir, { recursive: true });
          return respond({ removed: input.name });
        }
        case 'sync': {
          const projectRoot = directory ?? process.cwd();
          const result = syncSkills(projectRoot);
          return respond(result as unknown as Record<string, unknown>);
        }
        default:
          return errorResponse({ terminal: true, reason: 'unknown_action', error: `Unknown action: ${(input as { action: string }).action}` });
      }
    }),
  );
}

