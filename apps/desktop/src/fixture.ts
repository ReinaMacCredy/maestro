import type { RepoSnapshot } from "./model";

// Store snapshot from 2026-08-25 (maestro) plus one synthetic repo carrying a
// STALLED_LEASE finding so every card variant renders. Used by tests and by
// `vite dev` outside Tauri.
export const FIXTURE_NOW = new Date("2026-08-25T11:40:00Z");

const CODEX = "01a03893-3def-7643-a2ac-6078976a34ef";
const CLAUDE = "4763685d-6962-4abb-bd85-c372c1d58508";

const w = (
  id: string,
  title: string,
  state: "open" | "active" | "done" | "cancelled",
  parentId: string | null,
  heldBy: string | null = null,
) => ({ id, title, kind: "task", state, parentId, heldBy, updatedAt: "2026-08-25T11:37:13.571Z" });

export const MAESTRO: RepoSnapshot = {
  repo: "maestro",
  path: "/Users/reinamaccredy/Code/maestro",
  at: "2026-08-25T11:39:59Z",
  works: [
    w("w15", "SLP doctrine adoption: envelope/handback, evidence layers, attention, supervisor", "open", null),
    w("w16", "D1 text: maestro-work dispatch envelope, handback vocabulary", "done", "w15"),
    w("w17", "D1 text: maestro-verify evidence layers + handback fields", "done", "w15"),
    w("w18", "D1 text: maestro-design intake + council reconcile", "done", "w15"),
    w("w19", "D2 maestro attention: 4 detectors, attention table, lease-chain routing", "done", "w15"),
    w("w20", "D3 supervisor start|stop|status: detached tick loop, pid file", "done", "w15"),
    w("w21", "D3 PostToolUse delivery: install wires the event for both harnesses", "active", "w15", CODEX),
  ],
  ready: [],
  gated: [
    {
      id: "w15",
      title: "SLP doctrine adoption",
      blockers: [{ id: "w21", state: "active" }],
      reason: "w15 has open children: w21; finish them first: maestro status",
      command: "maestro status",
      origin: "policy-breakdown",
    },
  ],
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
  sessions: [
    { id: CODEX, harness: "codex", live: true, lastSeen: "2026-08-25T11:37:13Z" },
    { id: CLAUDE, harness: "claude", live: true, lastSeen: "2026-08-25T11:38:00Z" },
  ],
};

export const ASTRA: RepoSnapshot = {
  repo: "astra",
  path: "/Users/reinamaccredy/Code/astra",
  at: "2026-08-25T11:39:59Z",
  works: [
    w("w3", "plugin-engine: hot-reload SLP plugins without restart", "active", null, CLAUDE),
    w("w4", "web-debug recipe: capture console + network in one bundle", "open", null),
  ],
  ready: ["w4"],
  gated: [],
  decisions: [],
  findings: [
    {
      kind: "STALLED_LEASE",
      packet: "w3 active, holder 4763685d live, last_seen 34 min ago (threshold 30)",
      subjectSession: CLAUDE,
      subjectWork: "w3",
      targets: [],
      raisedAt: "2026-08-25T11:06:00Z",
      raised: true,
    },
  ],
  sessions: [{ id: CLAUDE, harness: "claude", live: true, lastSeen: "2026-08-25T11:06:00Z" }],
};

export const SYNARA: RepoSnapshot = {
  repo: "synara",
  path: "/Users/reinamaccredy/Code/synara",
  at: "2026-08-25T11:39:59Z",
  works: [],
  ready: [],
  gated: [],
  decisions: [],
  findings: [],
  sessions: [],
};

export const FIXTURE: RepoSnapshot[] = [MAESTRO, ASTRA, SYNARA];
