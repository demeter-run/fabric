use anyhow::{bail, Result};
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
        auth::Auth0Driven,
        project::cache::ProjectDrivenCacheBilling,
        usage::{UsageReport, UsageReportImpl},
    },
    driven::{
        auth0::Auth0DrivenImpl,
        cache::{project::SqliteProjectDrivenCache, usage::SqliteUsageDrivenCache, SqliteCache},
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

pub async fn fetch_projects(config: BillingConfig, email: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let auth0: Box<dyn Auth0Driven> = Box::new(
        Auth0DrivenImpl::try_new(
            &config.auth_url,
            &config.auth_client_id,
            &config.auth_client_secret,
            &config.auth_audience,
        )
        .await?,
    );
    let project_cache: Box<dyn ProjectDrivenCacheBilling> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    let Some(user) = auth0.find_info_by_email(email).await? else {
        bail!("Invalid email")
    };

    let projects = project_cache.find_by_user_id(&user.user_id).await?;
    if projects.is_empty() {
        bail!("No one project was found")
    }

    let mut table = Table::new();
    table.set_header(vec!["", "name", "namespace", "status", "createdAt"]);

    for (i, p) in projects.iter().enumerate() {
        table.add_row(vec![
            &(i + 1).to_string(),
            &p.name,
            &p.namespace,
            &p.status.to_string(),
            &p.created_at.to_rfc3339(),
        ]);
    }

    println!("{table}");

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
            &format!("${:.2}", r.units_cost.unwrap_or(0.)),
            &format!("${:.2}", r.minimum_cost.unwrap_or(0.)),
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
            &format!("${:.2}", r.units_cost.unwrap_or(0.)),
            &format!("${:.2}", r.minimum_cost.unwrap_or(0.)),
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

    pub auth_url: String,
    pub auth_client_id: String,
    pub auth_client_secret: String,
    pub auth_audience: String,
}
