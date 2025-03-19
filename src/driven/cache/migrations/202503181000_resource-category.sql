-- Add new column category to store the category of the resource
-- At the beginning all were demeter-port, now could be demeter-port or demeter-worker
ALTER TABLE resource ADD COLUMN category VARCHAR NOT NULL default 'demeter-port';

-- add new indexes
CREATE INDEX IF NOT EXISTS idx_resource_category ON resource(category);
CREATE INDEX IF NOT EXISTS idx_resource_project_id_status_category ON resource(project_id, status, category);