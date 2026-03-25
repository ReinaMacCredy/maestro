/**
 * Workflow event bus -- synchronous pub/sub for tool lifecycle events.
 * Emitted by MCP handlers after state transitions, consumed by hooks and engine.
 */

export type WorkflowEventType =
  | 'plan-approved'
  | 'tasks-synced'
  | 'task-claimed'
  | 'task-done'
  | 'task-failed-verification'
  | 'feature-complete'
  | 'tool-installed';

export interface WorkflowEvent {
  type: WorkflowEventType;
  timestamp: string;
  feature?: string;
  task?: string;
  metadata?: Record<string, unknown>;
}

export type WorkflowEventListener = (event: WorkflowEvent) => void;

export class WorkflowEventBus {
  private listeners = new Map<WorkflowEventType, Set<WorkflowEventListener>>();

  on(type: WorkflowEventType, listener: WorkflowEventListener): void {
    if (!this.listeners.has(type)) {
      this.listeners.set(type, new Set());
    }
    this.listeners.get(type)!.add(listener);
  }

  off(type: WorkflowEventType, listener: WorkflowEventListener): void {
    this.listeners.get(type)?.delete(listener);
  }

  emit(event: WorkflowEvent): void {
    const handlers = this.listeners.get(event.type);
    if (!handlers) return;
    for (const handler of handlers) {
      try { handler(event); } catch { /* best-effort */ }
    }
  }

  listenerCount(type: WorkflowEventType): number {
    return this.listeners.get(type)?.size ?? 0;
  }
}
