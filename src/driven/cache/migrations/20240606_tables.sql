CREATE TABLE IF NOT EXISTS project (
  id TEXT PRIMARY KEY NOT NULL,
  namespace TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  owner TEXT NOT NULL,
  status TEXT NOT NULL,
  billing_provider TEXT NOT NULL,
  billing_provider_id TEXT NOT NULL,
  billing_subscription_id TEXT,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS resource (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  spec TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL,
  FOREIGN KEY(project_id) REFERENCES project(id)
);

CREATE TABLE IF NOT EXISTS project_user (
  user_id TEXT NOT NULL,
  project_id TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at DATETIME NOT NULL,
  PRIMARY KEY (user_id, project_id),
  FOREIGN KEY(project_id) REFERENCES project(id)
);

CREATE TABLE IF NOT EXISTS project_user_invite (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL,
  email TEXT NOT NULL,
  role TEXT NOT NULL,
  code TEXT NOT NULL,
  status TEXT NOT NULL,
  expires_in DATETIME NOT NULL,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL,
  FOREIGN KEY(project_id) REFERENCES project(id)
);

CREATE TABLE IF NOT EXISTS project_secret (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL,
  name TEXT NOT NULL,
  phc TEXT NOT NULL,
  secret BLOB NOT NULL,
  created_at DATETIME NOT NULL,
  FOREIGN KEY(project_id) REFERENCES project(id)
);

CREATE TABLE IF NOT EXISTS usage (
  id TEXT PRIMARY KEY NOT NULL,
  event_id TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  units INT NOT NULL,
  tier TEXT NOT NULL,
  interval INT NOT NULL,
  created_at DATETIME NOT NULL,
  FOREIGN KEY(resource_id) REFERENCES resource(id)
);

