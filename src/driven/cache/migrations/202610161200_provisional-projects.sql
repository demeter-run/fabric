-- A provisional project has no owner until a person claims it, so the owner
-- becomes nullable. SQLite cannot drop a NOT NULL constraint in place, and
-- rebuilding the table fails inside the migration's transaction because the
-- other tables' foreign keys point at it. Swapping the column for a nullable
-- copy keeps every row and every foreign key.
ALTER TABLE project ADD COLUMN owner_nullable TEXT;
UPDATE project SET owner_nullable = owner;
ALTER TABLE project DROP COLUMN owner;
ALTER TABLE project RENAME COLUMN owner_nullable TO owner;

-- What an agent declares at registration and what the claim and the expiry
-- need. The claim token is stored only as its SHA-256, and the client IP only
-- as a salted hash.
ALTER TABLE project ADD COLUMN agent_provisioned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE project ADD COLUMN agent_name TEXT;
ALTER TABLE project ADD COLUMN agent_version TEXT;
ALTER TABLE project ADD COLUMN agent_url TEXT;
ALTER TABLE project ADD COLUMN expires_at TEXT;
ALTER TABLE project ADD COLUMN claim_token_hash TEXT;
ALTER TABLE project ADD COLUMN user_code TEXT;
ALTER TABLE project ADD COLUMN source_ip_hash TEXT;

CREATE INDEX IF NOT EXISTS idx_project_claim_token_hash ON project(claim_token_hash);
CREATE INDEX IF NOT EXISTS idx_project_user_code ON project(user_code);
CREATE INDEX IF NOT EXISTS idx_project_status_expires_at ON project(status, expires_at);
