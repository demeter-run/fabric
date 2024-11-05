use anyhow::{bail, Result};
use comfy_table::Table;
use include_dir::{include_dir, Dir};
use serde_json::json;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::error;

use crate::{
    domain::{
        auth::Auth0Driven,
        project::cache::ProjectDrivenCacheBilling,
        resource::cache::ResourceDrivenCacheBilling,
        usage::{cache::UsageDrivenCacheBilling, UsageReport, UsageReportImpl},
    },
    driven::{
        auth0::Auth0DrivenImpl,
        cache::{
            project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache,
            usage::SqliteUsageDrivenCache, SqliteCache,
        },
        metadata::FileMetadata,
    },
};

pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

static METADATA: Dir = include_dir!("bootstrap/rpc/crds");

pub async fn fetch_usage(config: BillingConfig, period: &str, output: OutputFormat) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let usage_cache: Box<dyn UsageDrivenCacheBilling> =
        Box::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    let metadata = Arc::new(FileMetadata::from_dir(METADATA.clone())?);

    let report = usage_cache
        .find_report_aggregated(period)
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

pub async fn fetch_resources(config: BillingConfig, project_namespace: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let billing_cache: Box<dyn ResourceDrivenCacheBilling> =
        Box::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

    let resouces = billing_cache
        .find_by_project_namespace(project_namespace)
        .await?;
    if resouces.is_empty() {
        bail!("No one resouce was found")
    }

    let mut table = Table::new();
    table.set_header(vec![
        "",
        "name",
        "kind",
        "status",
        "tier",
        "network",
        "createdAt",
    ]);

    for (i, r) in resouces.iter().enumerate() {
        let spec: serde_json::Value = serde_json::from_str(&r.spec).unwrap();

        let tier = match spec.get("throughputTier") {
            Some(v) => v.as_str().unwrap(),
            None => "unknown",
        };
        let network = match spec.get("network") {
            Some(v) => v.as_str().unwrap(),
            None => "unknown",
        };

        table.add_row(vec![
            &(i + 1).to_string(),
            &r.name,
            &r.kind,
            &r.status.to_string(),
            tier,
            network,
            &r.created_at.to_rfc3339(),
        ]);
    }

    println!("{table}");

    Ok(())
}

pub async fn fetch_new_users(config: BillingConfig, after: &str) -> Result<()> {
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

    let project_users = project_cache.find_new_users(after).await?;
    if project_users.is_empty() {
        bail!("No one new user was found")
    }

    let ids: Vec<String> = project_users.iter().map(|p| p.user_id.clone()).collect();
    let profiles = auth0.find_info_by_ids(&ids).await?;

    let mut table = Table::new();
    table.set_header(vec![
        "",
        "id",
        "name",
        "email",
        "role",
        "project",
        "stripe ID",
        "createdAt",
    ]);

    for (i, u) in project_users.iter().enumerate() {
        let (name, email) = match profiles.iter().find(|a| a.user_id == u.user_id) {
            Some(a) => (a.name.clone(), a.email.clone()),
            None => ("unknown".into(), "unknown".into()),
        };

        table.add_row(vec![
            &(i + 1).to_string(),
            &u.user_id,
            &name,
            &email,
            &u.role.to_string(),
            &u.project_namespace,
            &u.project_billing_provider_id,
            &u.created_at.to_rfc3339(),
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
