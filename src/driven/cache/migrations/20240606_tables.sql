CREATE TABLE IF NOT EXISTS projects (
  slug TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ports (
  id TEXT PRIMARY KEY NOT NULL,
  project TEXT NOT NULL,
  kind TEXT NOT NULL,
  data TEXT NOT NULL,
  FOREIGN KEY(project) REFERENCES projects(slug)
);

CREATE TABLE IF NOT EXISTS projects_users (
  user_id TEXT NOT NULL,
  project_id TEXT NOT NULL,
  PRIMARY KEY (user_id, project_id),
  FOREIGN KEY(project_id) REFERENCES projects(slug)
)
