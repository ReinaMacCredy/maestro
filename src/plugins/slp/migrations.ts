import { Store } from "../../kernel/store.ts";

export function migrateRoom(store: Store): void {
  store.migrate(`
    CREATE TABLE IF NOT EXISTS slp_teams (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      project_path TEXT NOT NULL,
      objective TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      state TEXT NOT NULL CHECK(state IN ('RUNNING', 'STOPPED')),
      workspace_id TEXT NOT NULL,
      created_at TEXT NOT NULL,
      stopped_at TEXT,
      PRIMARY KEY(team_id, generation),
      UNIQUE(project_path, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_team_roles (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('lead', 'peer')),
      name TEXT NOT NULL,
      pane_id TEXT NOT NULL,
      workspace_id TEXT NOT NULL,
      instance_id TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      brief_digest TEXT NOT NULL,
      ready_challenge TEXT NOT NULL,
      profile TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, name),
      FOREIGN KEY(team_id, generation) REFERENCES slp_teams(team_id, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_activity (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      actor TEXT NOT NULL,
      operation TEXT NOT NULL,
      target_type TEXT NOT NULL,
      target_id TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_lifecycle_operations (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      operation TEXT NOT NULL CHECK(operation IN ('START', 'STOP')),
      phase TEXT NOT NULL CHECK(phase IN ('RESERVED', 'RUNTIME_READY', 'COMMITTED')),
      revision INTEGER NOT NULL CHECK(revision > 0),
      project_path TEXT NOT NULL,
      objective TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      work_id TEXT NOT NULL,
      workspace_id TEXT,
      runtime_json TEXT,
      actor TEXT NOT NULL,
      reason TEXT NOT NULL DEFAULT '',
      emergency INTEGER NOT NULL CHECK(emergency IN (0, 1)),
      owner_token TEXT,
      owner_pid INTEGER,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, operation),
      UNIQUE(project_path, generation, operation)
    );
    CREATE TABLE IF NOT EXISTS slp_decisions (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      choice TEXT NOT NULL,
      why TEXT NOT NULL,
      scope TEXT NOT NULL,
      work_id TEXT,
      replaces_id TEXT,
      actor TEXT NOT NULL,
      created_at TEXT NOT NULL,
      provisional INTEGER NOT NULL DEFAULT 0
    );
  `);
}

export function migrateProject(store: Store): void {
  store.migrate(`
    CREATE TABLE IF NOT EXISTS slp_local_teams (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      room_store_path TEXT NOT NULL,
      project_path TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      state TEXT NOT NULL CHECK(state IN ('RUNNING', 'STOPPED')),
      workspace_id TEXT NOT NULL,
      bound_at TEXT NOT NULL,
      runtime_pane_id TEXT NOT NULL DEFAULT '',
      supervisor_pane_id TEXT NOT NULL DEFAULT '',
      PRIMARY KEY(team_id, generation)
    );
    CREATE TABLE IF NOT EXISTS slp_local_roles (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      role TEXT NOT NULL CHECK(role IN ('lead', 'peer')),
      name TEXT NOT NULL,
      pane_id TEXT NOT NULL,
      workspace_id TEXT NOT NULL,
      instance_id TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      brief_digest TEXT NOT NULL,
      ready_challenge TEXT NOT NULL,
      profile TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, name)
    );
    CREATE TABLE IF NOT EXISTS slp_work (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      objective TEXT NOT NULL,
      created_by TEXT NOT NULL,
      assigned_to TEXT NOT NULL,
      owner TEXT,
      state TEXT NOT NULL CHECK(state IN ('OPEN', 'ACTIVE', 'RETURNED', 'DONE')),
      current_return TEXT,
      return_revision INTEGER NOT NULL DEFAULT 0,
      abandoned_at TEXT,
      abandoned_by TEXT,
      abandonment_reason TEXT,
      acceptance_outcome TEXT,
      accepted_by TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_work_entries (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      kind TEXT NOT NULL CHECK(kind IN ('NOTE', 'RETURN', 'ACCEPTANCE')),
      actor TEXT NOT NULL,
      body TEXT NOT NULL,
      created_at TEXT NOT NULL,
      flag TEXT
    );
    CREATE TABLE IF NOT EXISTS slp_rework_grants (
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      return_revision INTEGER NOT NULL CHECK(return_revision > 0),
      reviewer TEXT NOT NULL,
      granted_at TEXT NOT NULL,
      consumed_at TEXT,
      PRIMARY KEY(work_id, return_revision)
    );
    CREATE TABLE IF NOT EXISTS slp_lesson_candidates (
      work_id TEXT NOT NULL REFERENCES slp_work(id),
      source TEXT NOT NULL,
      recorded_at TEXT NOT NULL,
      PRIMARY KEY(work_id, source)
    );
    CREATE TABLE IF NOT EXISTS slp_decisions (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      choice TEXT NOT NULL,
      why TEXT NOT NULL,
      scope TEXT NOT NULL,
      work_id TEXT,
      replaces_id TEXT,
      actor TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_activity (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      actor TEXT NOT NULL,
      operation TEXT NOT NULL,
      target_type TEXT NOT NULL,
      target_id TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS slp_lifecycle_operations (
      team_id TEXT NOT NULL,
      generation INTEGER NOT NULL,
      operation TEXT NOT NULL CHECK(operation IN ('START', 'STOP')),
      phase TEXT NOT NULL CHECK(phase IN ('RESERVED', 'RUNTIME_READY', 'COMMITTED')),
      revision INTEGER NOT NULL CHECK(revision > 0),
      project_path TEXT NOT NULL,
      objective TEXT NOT NULL,
      configuration_json TEXT NOT NULL,
      pack_version TEXT NOT NULL,
      pack_digest TEXT NOT NULL,
      work_id TEXT NOT NULL,
      workspace_id TEXT,
      runtime_json TEXT,
      actor TEXT NOT NULL,
      reason TEXT NOT NULL DEFAULT '',
      emergency INTEGER NOT NULL CHECK(emergency IN (0, 1)),
      owner_token TEXT,
      owner_pid INTEGER,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY(team_id, generation, operation),
      UNIQUE(project_path, generation, operation)
    );
  `);
}
