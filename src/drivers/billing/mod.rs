use anyhow::Result;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::info;

use crate::driven::cache::{
    project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache,
    usage::SqliteUsageDrivenCache, SqliteCache,
};

pub async fn run(config: BillingConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let _project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let _resource_cache = Arc::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));
    let _usage_cache = Arc::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    info!("Aggregating data");

    Ok(())
}

#[derive(Debug)]
pub struct BillingTlsConfig {
    pub ssl_crt_path: PathBuf,
    pub ssl_key_path: PathBuf,
}

#[derive(Debug)]
pub struct BillingConfig {
    pub db_path: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
    pub stripe_url: String,
    pub stripe_api_key: String,
    pub tls_config: Option<BillingTlsConfig>,
}
