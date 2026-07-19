CREATE TABLE IF NOT EXISTS vents (
  id             TEXT PRIMARY KEY,
  project_id     TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  message        TEXT NOT NULL,
  severity       TEXT NOT NULL DEFAULT 'medium',
  tags           TEXT NOT NULL DEFAULT '[]',
  topic_key      TEXT NOT NULL,
  seen_count     INTEGER NOT NULL DEFAULT 1,
  actionable     INTEGER NOT NULL DEFAULT 0,
  item_id        TEXT REFERENCES items(id) ON DELETE SET NULL,
  first_event_id TEXT NOT NULL,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  UNIQUE(project_id, topic_key)
);
CREATE INDEX IF NOT EXISTS idx_vents_project_actionable ON vents(project_id, actionable);
