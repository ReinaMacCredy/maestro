import type { TaskStatusType, FeatureStatusType, MemoryCategory } from '../../core/types.ts';
import type { ExecutionInsight, DoctrineEffectivenessInsight } from '../workflow/insights.ts';
import type { DoctrineItem } from '../../doctrine/port.ts';

// ============================================================================
// Template Contract
// ============================================================================

export interface TemplateInput<T> {
  data: T;
  title: string;
  feature?: string;
  generatedAt: string;
}

export interface TemplateOutput {
  bodyHtml: string;
  extraHead?: string;
  extraScripts?: string;
}

export type TemplateRenderer<T> = (input: TemplateInput<T>) => TemplateOutput;

// ============================================================================
// Visual Type Enums
// ============================================================================

export type MaestroVisualType =
  | 'plan-graph'
  | 'status-dashboard'
  | 'memory-map'
  | 'execution-timeline'
  | 'doctrine-network';

export type DebugVisualType =
  | 'component-tree'
  | 'state-flow'
  | 'error-cascade'
  | 'network-waterfall'
  | 'dom-diff'
  | 'console-timeline';

export type VisualType = MaestroVisualType | DebugVisualType;

export const MAESTRO_VISUAL_TYPES: readonly MaestroVisualType[] = [
  'plan-graph', 'status-dashboard', 'memory-map', 'execution-timeline', 'doctrine-network',
] as const;

export const DEBUG_VISUAL_TYPES: readonly DebugVisualType[] = [
  'component-tree', 'state-flow', 'error-cascade', 'network-waterfall', 'dom-diff', 'console-timeline',
] as const;

// ============================================================================
// Visual Result
// ============================================================================

export interface VisualResult {
  path: string;
  opened: boolean;
  type: VisualType;
  feature?: string;
}

// ============================================================================
// Maestro Visual Data (gathered from services)
// ============================================================================

export interface PlanGraphTask {
  id: string;
  name: string;
  status: TaskStatusType;
  dependsOn: string[];
  claimedBy?: string;
  summary?: string;
}

export interface PlanGraphData {
  tasks: PlanGraphTask[];
  planContent?: string;
  feature: string;
}

export interface DoctrineStats {
  total: number;
  active: number;
  deprecated: number;
}

export interface StatusDashboardData {
  feature: {
    name: string;
    status: FeatureStatusType;
    createdAt: string;
    approvedAt?: string;
    completedAt?: string;
  };
  tasks: {
    total: number;
    pending: number;
    claimed: number;
    done: number;
    blocked: number;
    review: number;
    revision: number;
  };
  runnable: string[];
  blocked: string[];
  pipelineStage: string;
  memoryStats: { count: number; totalBytes: number };
  doctrineStats: DoctrineStats;
  nextAction: string;
}

export interface MemoryMapEntry {
  name: string;
  category?: MemoryCategory;
  priority?: number;
  tags: string[];
  sizeBytes: number;
  updatedAt: string;
}

export interface MemoryMapData {
  memories: MemoryMapEntry[];
  feature: string;
}

export interface ExecutionTimelineData {
  insights: ExecutionInsight[];
  knowledgeFlow: Array<{ from: string; to: string; proximity: number }>;
  coverage: { totalTasks: number; withExecMemory: number; percent: number };
  doctrineEffectiveness?: DoctrineEffectivenessInsight[];
  feature: string;
}

export interface DoctrineNetworkData {
  items: DoctrineItem[];
  feature: string;
}

// ============================================================================
// Debug Visual Data (agent-provided)
// ============================================================================

export interface ComponentTreeNode {
  id: string;
  name: string;
  type: 'component' | 'element' | 'provider' | 'fragment';
  props?: Record<string, unknown>;
  children?: string[];
  error?: string;
  errorBoundary?: boolean;
}

export interface ComponentTreeData {
  nodes: ComponentTreeNode[];
}

export interface StateFlowEntry {
  timestamp: string;
  action: string;
  prevState: Record<string, unknown>;
  nextState: Record<string, unknown>;
  source?: string;
}

export interface StateFlowData {
  timeline: StateFlowEntry[];
}

export interface ErrorCascadeEntry {
  id: string;
  message: string;
  stack?: string;
  boundary?: string;
  caught?: boolean;
  children?: string[];
}

export interface ErrorCascadeData {
  errors: ErrorCascadeEntry[];
}

export interface NetworkRequest {
  id: string;
  url: string;
  method: string;
  startTime: number;
  endTime: number;
  status: number;
  size?: number;
  error?: string;
}

export interface NetworkWaterfallData {
  requests: NetworkRequest[];
}

export interface DomDiffData {
  expected: string;
  actual: string;
  context?: string;
}

export interface ConsoleEntry {
  timestamp: string;
  level: 'log' | 'warn' | 'error' | 'info' | 'debug';
  message: string;
  data?: unknown;
  source?: string;
}

export interface ConsoleTimelineData {
  entries: ConsoleEntry[];
}
