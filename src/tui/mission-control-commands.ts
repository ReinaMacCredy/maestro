import type { MissionControlMode } from "./types.js";

export type MissionControlCommandId =
  | "features"
  | "handoffs"
  | "config"
  | "processes"
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

  return [
    featureCommand,
    {
        id: "handoffs",
        key: "H",
        label: "Handoffs",
        detail: "Review pending cross-agent handoffs",
        section: "Navigate",
        keywords: ["handoff", "handoffs", "agent"],
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
        id: "processes",
        key: "P",
        label: "Runtime",
        detail: "List live Maestro runtime work for this mission",
        section: "Navigate",
        keywords: ["processes", "runtime", "workers"],
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
