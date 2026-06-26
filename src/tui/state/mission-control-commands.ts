import type { MissionControlMode } from "./types.js";

export type MissionControlCommandId =
  | "features"
  | "dependencies"
  | "config"
  | "memory"
  | "graph"
  | "agent-grid"
  | "dispatch"
  | "event-stream"
  | "task-board"
  | "timeline"
  | "principle-review"
  | "help"
  | "exit";

export interface MissionControlCommandSpec {
  readonly id: MissionControlCommandId;
  readonly key: string;
  readonly label: string;
  readonly detail: string;
  readonly section: "Navigate" | "Session";
  readonly keywords: readonly string[];
}

export function getMissionControlCommandSpecs(
  mode: MissionControlMode,
): readonly MissionControlCommandSpec[] {
  const featureCommand: MissionControlCommandSpec = mode === "home"
    ? {
        id: "features",
        key: "F",
        label: "Overview",
        detail: "Show the guided Mission Control home screen",
        section: "Navigate",
        keywords: ["overview", "home", "project", "empty state"],
    }
    : {
        id: "features",
        key: "F",
        label: "Tasks",
        detail: "Browse mission tasks and focus a specific item",
        section: "Navigate",
        keywords: ["tasks", "features", "feature browser", "focus"],
      };

  if (mode === "home") {
    return [
      featureCommand,
      {
        id: "agent-grid",
        key: "A",
        label: "Agents",
        detail: "View agent status and activity",
        section: "Navigate",
        keywords: ["agents", "status", "grid"],
      },
      {
        id: "event-stream",
        key: "E",
        label: "Events",
        detail: "Browse the event timeline",
        section: "Navigate",
        keywords: ["events", "timeline", "stream", "log"],
      },
      {
        id: "task-board",
        key: "T",
        label: "Tasks",
        detail: "Kanban-style task board with status columns",
        section: "Navigate",
        keywords: ["tasks", "board", "kanban", "issues"],
      },
      {
        id: "config",
        key: "C",
        label: "Config",
        detail: "Inspect workspace configuration, checks, and mission directory",
        section: "Navigate",
        keywords: ["config", "configuration", "doctor", "directory"],
      },
      {
        id: "memory",
        key: "M",
        label: "Memory",
        detail: "View corrections, learnings, and ratchet assertions",
        section: "Navigate",
        keywords: ["memory", "corrections", "learnings", "ratchet"],
      },
      {
        id: "graph",
        key: "G",
        label: "Graph",
        detail: "View cross-project relationships",
        section: "Navigate",
        keywords: ["graph", "projects", "relationships"],
      },
      {
        id: "principle-review",
        key: "R",
        label: "Principles",
        detail: "Review principle effectiveness and outcomes",
        section: "Navigate",
        keywords: ["principles", "principle", "gates", "effectiveness", "outcomes", "scoreboard", "helpful", "unhelpful"],
      },
      {
        id: "help",
        key: "?",
        label: "Help",
        detail: "Show keyboard shortcuts and available commands",
        section: "Session",
        keywords: ["help", "keys", "shortcuts", "hotkeys"],
      },
      {
        id: "exit",
        key: "Ctrl+T",
        label: "Exit",
        detail: "Close Mission Control cleanly",
        section: "Session",
        keywords: ["quit", "exit", "close"],
      },
    ];
  }

  return [
      featureCommand,
      {
        id: "agent-grid",
        key: "A",
        label: "Agents",
        detail: "View agent status and activity",
        section: "Navigate",
        keywords: ["agents", "status", "grid"],
      },
      {
        id: "dispatch",
        key: "D",
        label: "Dispatch",
        detail: "Prepare and assign ready features to agents",
        section: "Navigate",
        keywords: ["dispatch", "assign", "queue", "ready"],
      },
      {
        id: "event-stream",
        key: "E",
        label: "Events",
        detail: "Browse the event timeline",
        section: "Navigate",
        keywords: ["events", "timeline", "stream", "log"],
      },
      {
        id: "dependencies",
        key: "B",
        label: "Dependencies",
        detail: "Inspect blockers and downstream tasks for the selected item",
        section: "Navigate",
        keywords: ["dependencies", "blocked by", "graph", "unblocks"],
      },
      {
        id: "timeline",
        key: "L",
        label: "Timeline",
        detail: "View milestone progress and feature assignments",
        section: "Navigate",
        keywords: ["timeline", "milestones", "progress"],
      },
      {
        id: "task-board",
        key: "T",
        label: "Tasks",
        detail: "Kanban-style task board with status columns",
        section: "Navigate",
        keywords: ["tasks", "board", "kanban", "issues"],
      },
      {
        id: "config",
        key: "C",
        label: "Config",
        detail: "Inspect workspace configuration, checks, and mission directory",
        section: "Navigate",
        keywords: ["config", "configuration", "doctor", "directory"],
      },
      {
        id: "memory",
        key: "M",
        label: "Memory",
        detail: "View corrections, learnings, and ratchet assertions",
        section: "Navigate",
        keywords: ["memory", "corrections", "learnings", "ratchet"],
      },
      {
        id: "graph",
        key: "G",
        label: "Graph",
        detail: "View cross-project relationships",
        section: "Navigate",
        keywords: ["graph", "projects", "relationships"],
      },
      {
        id: "principle-review",
        key: "R",
        label: "Principles",
        detail: "Review principle effectiveness and outcomes",
        section: "Navigate",
        keywords: ["principles", "principle", "gates", "effectiveness", "outcomes", "scoreboard", "helpful", "unhelpful"],
      },
      {
        id: "help",
        key: "?",
        label: "Help",
        detail: "Show keyboard shortcuts and available commands",
        section: "Session",
        keywords: ["help", "keys", "shortcuts", "hotkeys"],
      },
      {
        id: "exit",
        key: "Ctrl+T",
        label: "Exit",
        detail: "Close Mission Control cleanly",
        section: "Session",
        keywords: ["quit", "exit", "close"],
      },
  ];
}

export function getFilteredMissionControlCommandSpecs(
  mode: MissionControlMode,
  query: string,
): readonly MissionControlCommandSpec[] {
  const normalizedQuery = query.trim().toLowerCase();
  const commands = getMissionControlCommandSpecs(mode);
  if (normalizedQuery.length === 0) return commands;

  return commands.filter((command) =>
    [command.label, command.detail, command.section, ...command.keywords]
      .join(" ")
      .toLowerCase()
      .includes(normalizedQuery)
  );
}

export function getMissionControlPaletteCommandCount(mode: MissionControlMode): number {
  return getMissionControlCommandSpecs(mode).length;
}

export function getFilteredMissionControlPaletteCommandCount(
  mode: MissionControlMode,
  query: string,
): number {
  return getFilteredMissionControlCommandSpecs(mode, query).length;
}
