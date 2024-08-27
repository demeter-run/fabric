CREATE TABLE IF NOT EXISTS project_payment (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL UNIQUE,
  provider TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  subscription_id TEXT,
  created_at DATETIME NOT NULL,
  FOREIGN KEY(project_id) REFERENCES project(id)
);

