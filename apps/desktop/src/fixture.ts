import type { Holder, RepoSnapshot, Work } from "./model";

// Store snapshot from 2026-08-25 (maestro) plus one synthetic repo carrying a
// REPEATED_FAILURE finding so every card variant renders. Used by tests and by
// `vite dev` outside Tauri.
export const FIXTURE_NOW = new Date("2026-08-25T11:40:00Z");

const CODEX: Holder = { id: "01a03893-3def-7643-a2ac-6078976a34ef", harness: "codex", live: true, lastSeen: "2026-08-25T11:37:13Z" };
const CLAUDE: Holder = { id: "4763685d-6962-4abb-bd85-c372c1d58508", harness: "claude", live: true, lastSeen: "2026-08-25T11:06:00Z" };

const w = (
  id: string,
  title: string,
  state: "open" | "active",
  parentId: string | null,
  extra: Partial<Work> = {},
): Work => ({ id, title, kind: "task", state, parentId, updatedAt: "2026-08-25T11:37:13.571Z", blockedBy: [], holder: null, gate: null, ...extra });

export const MAESTRO: RepoSnapshot = {
  repo: "maestro",
  path: "/Users/reinamaccredy/Code/maestro",
  at: "2026-08-25T11:39:59Z",
  works: [
    w("w15", "SLP doctrine adoption: envelope/handback, evidence layers, attention, supervisor", "open", null, {
      gate: {
        origin: "policy-breakdown",
        reason: "w15 has open children: w21; finish them first: maestro work list --parent w15",
        command: "maestro work list --parent w15",
      },
    }),
    w("w21", "D3 PostToolUse delivery: install wires the event for both harnesses", "active", "w15", { holder: CODEX }),
  ],
  counts: { open: 1, active: 1, done: 5, cancelled: 0 },
  decisions: [
    {
      id: "d10",
      text: "desktop shell: Tauri v2, macOS + Windows, pill anchored bottom-center of work_area",
      rationale: "Windows in scope drops the Swift shell; Electron rejected on ~150MB idle for a pill.",
      state: "draft",
      workId: null,
      updatedAt: "2026-08-25T11:31:00Z",
    },
    {
      id: "d3",
      text: "attention routes to holder(parent(w)) first",
      rationale: null,
      state: "locked",
      workId: "w15",
      updatedAt: "2026-08-25T10:50:39Z",
    },
  ],
  findings: [],
};

export const ASTRA: RepoSnapshot = {
  repo: "astra",
  path: "/Users/reinamaccredy/Code/astra",
  at: "2026-08-25T11:39:59Z",
  works: [
    w("w3", "plugin-engine: hot-reload SLP plugins without restart", "active", null, { holder: CLAUDE }),
    w("w4", "web-debug recipe: capture console + network in one bundle", "open", null),
    w("w5", "plugin-engine docs", "open", null, { blockedBy: [{ id: "w3", state: "active" }] }),
  ],
  counts: { open: 2, active: 1, done: 0, cancelled: 0 },
  decisions: [],
  findings: [
    {
      kind: "REPEATED_FAILURE",
      packet: "attention REPEATED_FAILURE work w3\n  observed: 3 failed passes since the latest lease\n  smallest action: maestro work show w3",
      subjectSession: CLAUDE.id,
      subjectWork: "w3",
      targets: [],
      raisedAt: "2026-08-25T11:06:00Z",
      raised: true,
    },
  ],
};

export const SYNARA: RepoSnapshot = {
  repo: "synara",
  path: "/Users/reinamaccredy/Code/synara",
  at: "2026-08-25T11:39:59Z",
  works: [],
  counts: { open: 0, active: 0, done: 0, cancelled: 0 },
  decisions: [],
  findings: [],
};

export const FIXTURE: RepoSnapshot[] = [MAESTRO, ASTRA, SYNARA];
