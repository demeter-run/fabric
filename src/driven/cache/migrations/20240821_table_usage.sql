CREATE TABLE IF NOT EXISTS usage (
  id TEXT PRIMARY KEY NOT NULL,
  event_id TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  units INT NOT NULL,
  created_at DATETIME NOT NULL,
  FOREIGN KEY(resource_id) REFERENCES resource(id)
);
