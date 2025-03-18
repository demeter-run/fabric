-- project_user
CREATE INDEX IF NOT EXISTS idx_project_user_user_id ON project_user(user_id);
CREATE INDEX IF NOT EXISTS idx_project_user_project_id ON project_user(project_id);
CREATE INDEX IF NOT EXISTS idx_project_user_created_at ON project_user(created_at);
CREATE INDEX IF NOT EXISTS idx_project_user_user_id_project_id ON project_user(user_id, project_id);

-- project
CREATE INDEX IF NOT EXISTS idx_project_status ON project(status);
CREATE INDEX IF NOT EXISTS idx_project_created_at ON project(created_at);
CREATE INDEX IF NOT EXISTS idx_project_status_namespace ON project(status, namespace);

-- project_secret
CREATE INDEX IF NOT EXISTS idx_project_secret_project_id ON project_secret(project_id);
CREATE INDEX IF NOT EXISTS idx_project_secret_created_at ON project_secret(created_at);

-- project_user_invite
CREATE INDEX IF NOT EXISTS idx_project_user_invite_project_expires_in_status ON project_user_invite(project_id, expires_in, status);
CREATE INDEX IF NOT EXISTS idx_project_user_invite_code ON project_user_invite(code);

-- resource
CREATE INDEX IF NOT EXISTS idx_resource_project_id ON resource(project_id);
CREATE INDEX IF NOT EXISTS idx_resource_created_at ON resource(created_at);
CREATE INDEX IF NOT EXISTS idx_resource_status ON resource(status);
CREATE INDEX IF NOT EXISTS idx_resource_project_id_status ON resource(project_id, status);

-- usage
CREATE INDEX IF NOT EXISTS idx_usage_created_at ON usage(created_at);
CREATE INDEX IF NOT EXISTS idx_usage_resource_id ON usage(resource_id);
CREATE INDEX IF NOT EXISTS idx_usage_tier ON usage(tier);
