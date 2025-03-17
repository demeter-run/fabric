ALTER TABLE "usage" ADD COLUMN cluster_id TEXT;

CREATE INDEX idx_usage_cluster_id ON usage(cluster_id);
