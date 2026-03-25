import { z } from 'zod';
import type { DebugVisualType, VisualResult, TemplateRenderer } from './types.ts';
import { renderPage, writeVisual } from '../../infra/visual/renderer.ts';
import { renderComponentTree } from '../../infra/visual/templates/component-tree.ts';
import { renderStateFlow } from '../../infra/visual/templates/state-flow.ts';
import { renderErrorCascade } from '../../infra/visual/templates/error-cascade.ts';
import { renderNetworkWaterfall } from '../../infra/visual/templates/network-waterfall.ts';
import { renderDomDiff } from '../../infra/visual/templates/dom-diff.ts';
import { renderConsoleTimeline } from '../../infra/visual/templates/console-timeline.ts';
import { MaestroError } from '../../domain/errors.ts';

// ============================================================================
// Zod Schemas
// ============================================================================

const ComponentTreeSchema = z.object({
  nodes: z.array(z.object({
    id: z.string().max(200),
    name: z.string().max(1000),
    type: z.enum(['component', 'element', 'provider', 'fragment']),
    props: z.record(z.unknown()).optional(),
    children: z.array(z.string().max(200)).max(100).optional(),
    error: z.string().max(10000).optional(),
    errorBoundary: z.boolean().optional(),
  })).max(500),
});

const StateFlowSchema = z.object({
  timeline: z.array(z.object({
    timestamp: z.string().max(100),
    action: z.string().max(500),
    prevState: z.record(z.unknown()),
    nextState: z.record(z.unknown()),
    source: z.string().max(200).optional(),
  })).max(1000),
});

const ErrorCascadeSchema = z.object({
  errors: z.array(z.object({
    id: z.string().max(200),
    message: z.string().max(10000),
    stack: z.string().max(50000).optional(),
    boundary: z.string().max(200).optional(),
    caught: z.boolean().optional(),
    children: z.array(z.string().max(200)).max(100).optional(),
  })).max(500),
});

const NetworkWaterfallSchema = z.object({
  requests: z.array(z.object({
    id: z.string().max(200),
    url: z.string().max(2000),
    method: z.string().max(10),
    startTime: z.number().finite(),
    endTime: z.number().finite(),
    status: z.number().int().min(0).max(999),
    size: z.number().min(0).optional(),
    error: z.string().max(10000).optional(),
  })).max(1000),
});

const DomDiffSchema = z.object({
  expected: z.string().max(500000),
  actual: z.string().max(500000),
  context: z.string().max(500).optional(),
});

const ConsoleTimelineSchema = z.object({
  entries: z.array(z.object({
    timestamp: z.string().max(100),
    level: z.enum(['log', 'warn', 'error', 'info', 'debug']),
    message: z.string().max(10000),
    data: z.unknown().optional(),
    source: z.string().max(200).optional(),
  })).max(5000),
});

const SCHEMAS: Record<DebugVisualType, z.ZodType> = {
  'component-tree': ComponentTreeSchema,
  'state-flow': StateFlowSchema,
  'error-cascade': ErrorCascadeSchema,
  'network-waterfall': NetworkWaterfallSchema,
  'dom-diff': DomDiffSchema,
  'console-timeline': ConsoleTimelineSchema,
};

// ============================================================================
// Template Dispatch
// ============================================================================

const RENDERERS: Record<DebugVisualType, TemplateRenderer<unknown>> = {
  'component-tree': renderComponentTree as TemplateRenderer<unknown>,
  'state-flow': renderStateFlow as TemplateRenderer<unknown>,
  'error-cascade': renderErrorCascade as TemplateRenderer<unknown>,
  'network-waterfall': renderNetworkWaterfall as TemplateRenderer<unknown>,
  'dom-diff': renderDomDiff as TemplateRenderer<unknown>,
  'console-timeline': renderConsoleTimeline as TemplateRenderer<unknown>,
};

// ============================================================================
// Main
// ============================================================================

export async function debugVisualize(
  type: DebugVisualType,
  data: unknown,
  title?: string,
  autoOpen: boolean = true,
): Promise<VisualResult> {
  // Validate
  const schema = SCHEMAS[type];
  if (!schema) {
    throw new MaestroError(`Unknown debug visualization type: ${type}`);
  }

  const result = schema.safeParse(data);
  if (!result.success) {
    throw new MaestroError(
      `Invalid data for ${type}: ${result.error.message}`,
      ['See maestro skill maestro:visual for schema reference'],
    );
  }

  // Render
  const renderer = RENDERERS[type];
  const pageTitle = title ?? type;
  const generatedAt = new Date().toISOString();
  const output = renderer({ data: result.data, title: pageTitle, generatedAt });

  const html = renderPage({
    title: pageTitle,
    bodyHtml: output.bodyHtml,
    extraHead: output.extraHead,
    extraScripts: output.extraScripts,
  });

  return writeVisual(type, html, undefined, autoOpen);
}
