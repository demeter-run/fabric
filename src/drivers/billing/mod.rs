use anyhow::Result;
use comfy_table::Table;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::info;

use crate::{
    domain,
    driven::cache::{usage::SqliteUsageDrivenCache, SqliteCache},
};

pub async fn run(config: BillingConfig, period: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let usage_cache = Arc::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    info!("Collecting data");
    let report = domain::usage::cache::find_report_aggregated(usage_cache.clone(), period).await?;
    let projects = report
        .iter()
        .map(|r| r.project_id.clone())
        .collect::<HashSet<_>>();

    let mut table = Table::new();
    table.set_header(vec!["project", "stripe_id", "port", "tier", "units"]);
    for id in projects {
        let resources: Vec<_> = report.iter().filter(|r| r.project_id == id).collect();
        let total_units: i64 = resources.iter().map(|r| r.units).sum();

        table.add_row(vec![
            &resources.first().unwrap().project_namespace,
            &resources.first().unwrap().project_billing_provider_id,
            "",
            "",
            &total_units.to_string(),
        ]);
        for r in resources {
            table.add_row(vec![
                "",
                "",
                &r.resource_kind,
                &r.tier,
                &r.units.to_string(),
            ]);
        }
    }

    println!("{table}");

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
