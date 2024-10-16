use anyhow::Result;
use comfy_table::Table;
use include_dir::{include_dir, Dir};
use serde_json::json;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{error, info};

use crate::{
    domain::{
        self,
        usage::{UsageReport, UsageReportImpl},
    },
    driven::{
        cache::{usage::SqliteUsageDrivenCache, SqliteCache},
        metadata::FileMetadata,
    },
};

pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

static METADATA: Dir = include_dir!("bootstrap/rpc/crds");

pub async fn run(config: BillingConfig, period: &str, output: OutputFormat) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let usage_cache = Arc::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    let metadata = Arc::new(FileMetadata::from_dir(METADATA.clone())?);

    info!("Collecting data");

    let report = domain::usage::cache::find_report_aggregated(usage_cache.clone(), period)
        .await?
        .calculate_cost(metadata.clone());

    match output {
        OutputFormat::Table => table(report),
        OutputFormat::Json => json(report),
        OutputFormat::Csv => csv(report, period),
    };

    Ok(())
}

fn csv(report: Vec<UsageReport>, period: &str) {
    let path = format!("{period}.csv");
    let result = csv::Writer::from_path(&path);
    if let Err(error) = result {
        error!(?error);
        return;
    }

    let mut wtr = result.unwrap();

    let result = wtr.write_record([
        "",
        "project",
        "stripe_id",
        "kind",
        "name",
        "tier",
        "time",
        "units",
        "units_cost",
        "minimum_cost",
    ]);
    if let Err(error) = result {
        error!(?error);
        return;
    }

    for (i, r) in report.iter().enumerate() {
        let result = wtr.write_record([
            &(i + 1).to_string(),
            &r.project_namespace,
            &r.project_billing_provider_id,
            &r.resource_kind,
            &r.resource_name,
            &r.tier,
            &format!("{:.1}h", ((r.interval as f64) / 60.) / 60.),
            &r.units.to_string(),
            &format!("${:.3}", r.units_cost.unwrap_or(0.)),
            &format!("${:.3}", r.minimum_cost.unwrap_or(0.)),
        ]);
        if let Err(error) = result {
            error!(?error);
            return;
        }
    }

    let result = wtr.flush();
    if let Err(error) = result {
        error!(?error);
        return;
    }

    println!("File {} created", path)
}

fn json(report: Vec<UsageReport>) {
    let mut json = vec![];

    for r in report {
        json.push(json!({
            "project_id": r.project_id,
            "project_namespace": r.project_namespace,
            "stripe_id": r.project_billing_provider_id,
            "resource_id": r.resource_id,
            "resource_kind": r.resource_kind,
            "resource_name": r.resource_name,
            "tier": r.tier,
            "interval": r.interval,
            "units": r.units,
            "units_cost": r.units_cost.unwrap_or(0.),
            "minimum_cost": r.minimum_cost.unwrap_or(0.),
        }))
    }

    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}

fn table(report: Vec<UsageReport>) {
    let mut table = Table::new();
    table.set_header(vec![
        "",
        "project",
        "stripe_id",
        "kind",
        "name",
        "tier",
        "time",
        "units",
        "units_cost",
        "minimum_cost",
    ]);

    for (i, r) in report.iter().enumerate() {
        table.add_row(vec![
            &(i + 1).to_string(),
            &r.project_namespace,
            &r.project_billing_provider_id,
            &r.resource_kind,
            &r.resource_name,
            &r.tier,
            &format!("{:.1}h", ((r.interval as f64) / 60.) / 60.),
            &r.units.to_string(),
            &format!("${:.3}", r.units_cost.unwrap_or(0.)),
            &format!("${:.3}", r.minimum_cost.unwrap_or(0.)),
        ]);
    }

    println!("{table}");
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
    pub tls_config: Option<BillingTlsConfig>,
}
