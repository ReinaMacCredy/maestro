export type TaskStatus = "pending" | "in_progress" | "completed";
export type TaskPriority = "low" | "normal" | "high";

export const TASK_STATUSES = ["pending", "in_progress", "completed"] as const;

export interface Task {
  readonly id: string;
  readonly missionId?: string;
  readonly title: string;
  readonly status: TaskStatus;
  readonly priority?: TaskPriority;
  readonly assignee?: string;
  readonly labels?: readonly string[];
}

export interface TaskQueryPort {
  readonly list?: () => Promise<readonly Task[]>;
  readonly all: () => Promise<readonly Task[]>;
}
