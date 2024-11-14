use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use comfy_table::Table;
use include_dir::{include_dir, Dir};
use kube::ResourceExt;
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
        metadata::MetadataDriven,
        project::{cache::ProjectDrivenCacheBackoffice, ProjectStatus},
        resource::{
            cache::ResourceDrivenCacheBackoffice, cluster::ResourceDrivenClusterBackoffice,
        },
        usage::{cache::UsageDrivenCacheBackoffice, UsageReport, UsageReportImpl},
    },
    driven::{
        auth0::Auth0DrivenImpl,
        cache::{
            project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache,
            usage::SqliteUsageDrivenCache, SqliteCache,
        },
        k8s::K8sCluster,
        metadata::FileMetadata,
    },
};

static METADATA: Dir = include_dir!("bootstrap/rpc/crds");

pub async fn fetch_usage(
    config: BackofficeConfig,
    period: &str,
    output: OutputFormat,
) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let usage_cache: Box<dyn UsageDrivenCacheBackoffice> =
        Box::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    let metadata = Arc::new(FileMetadata::from_dir(METADATA.clone())?);

    let report = usage_cache
        .find_report_aggregated(period)
        .await?
        .calculate_cost(metadata.clone());

    match output {
        OutputFormat::Table => output_table_usage(report),
        OutputFormat::Json => output_json_usage(report),
        OutputFormat::Csv => output_csv_usage(report, period),
    };

    Ok(())
}

pub async fn fetch_projects(
    config: BackofficeConfig,
    namespace: Option<String>,
    spec: Option<String>,
    resource: Option<String>,
    email: Option<String>,
) -> Result<()> {
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
    let project_cache: Box<dyn ProjectDrivenCacheBackoffice> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    if namespace.is_some() || spec.is_some() || resource.is_some() {
        let mut projects = Vec::new();

        if let Some(namespace) = namespace {
            if let Some(project) = project_cache.find_by_namespace(&namespace).await? {
                projects.push(project)
            }
        }

        if let Some(spec) = spec {
            projects.append(&mut project_cache.find_by_resource_spec(&spec).await?);
        }

        if let Some(resource) = resource {
            projects.append(&mut project_cache.find_by_resource_name(&resource).await?);
        }

        if projects.is_empty() {
            bail!("No one project was found")
        }

        let query = projects
            .iter()
            .map(|p| format!("user_id:{}", p.owner))
            .collect::<Vec<String>>()
            .join(" OR ");

        let profiles = auth0.find_info(&query).await?;

        let projects = projects
            .into_iter()
            .map(|p| {
                let email = profiles
                    .iter()
                    .find(|a| a.user_id == p.owner)
                    .map(|a| a.email.clone())
                    .unwrap_or("unknown".into());

                ProjectTable {
                    name: p.name,
                    namespace: p.namespace,
                    email,
                    status: p.status,
                    billing_provider_id: p.billing_provider_id,
                    created_at: p.created_at,
                }
            })
            .collect();

        output_table_project(projects);
        return Ok(());
    }

    if let Some(email) = email {
        let profile = auth0.find_info(&format!("email:{email}")).await?;
        if profile.is_empty() {
            bail!("No one user was found")
        };

        let profile = profile.first().unwrap();

        let projects = project_cache.find_by_user_id(&profile.user_id).await?;
        if projects.is_empty() {
            bail!("No one project was found")
        }

        let projects = projects.into_iter().map(|p| ProjectTable {
            name: p.name,
            namespace: p.namespace,
            email: profile.email.clone(),
            status: p.status,
            billing_provider_id: p.billing_provider_id,
            created_at: p.created_at,
        });

        output_table_project(projects.collect());
    }

    Ok(())
}

pub async fn fetch_resources(
    config: BackofficeConfig,
    project_namespace: Option<String>,
    spec: Option<String>,
) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let backoffice_cache: Box<dyn ResourceDrivenCacheBackoffice> =
        Box::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

    let resources = match (project_namespace, spec) {
        (None, Some(spec)) => backoffice_cache.find_by_spec(&spec).await?,
        (Some(namespace), None) => {
            backoffice_cache
                .find_by_project_namespace(&namespace)
                .await?
        }
        (Some(namespace), Some(_)) => {
            backoffice_cache
                .find_by_project_namespace(&namespace)
                .await?
        }
        (None, None) => bail!("No one resouce was found"),
    };
    if resources.is_empty() {
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

    for (i, r) in resources.iter().enumerate() {
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

pub async fn fetch_new_users(config: BackofficeConfig, after: &str) -> Result<()> {
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
    let project_cache: Box<dyn ProjectDrivenCacheBackoffice> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    let project_users = project_cache.find_new_users(after).await?;
    if project_users.is_empty() {
        bail!("No one new user was found")
    }

    let ids: Vec<String> = project_users
        .iter()
        .map(|p| format!("user_id:{}", p.user_id.clone()))
        .collect();
    let query = ids.join(" OR ");
    let profiles = auth0.find_info(&query).await?;

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

pub async fn fetch_diff(config: BackofficeConfig, output: OutputFormat) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let backoffice_cache: Box<dyn ResourceDrivenCacheBackoffice> =
        Box::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

    let backoffice_cluster: Box<dyn ResourceDrivenClusterBackoffice> =
        Box::new(K8sCluster::new().await?);

    let metadata = Arc::new(FileMetadata::from_dir(METADATA.clone())?);
    let resources_state = backoffice_cache.find_actives().await?;
    let mut resources_cluster = Vec::new();

    for metadata_resource in metadata.find()? {
        let kind = metadata_resource.crd.spec.names.kind;
        let mut items = backoffice_cluster.find_all(&kind).await?;
        resources_cluster.append(&mut items);
    }

    let mut report: HashMap<String, (bool, bool)> = HashMap::new();

    for resource in resources_state.iter() {
        let exist = resources_cluster.iter().any(|d| {
            let namespace = d.metadata.namespace.as_ref().unwrap().replace("prj-", "");
            let name = d.name_any();

            namespace.eq(&resource.project_namespace) && name.eq(&resource.name)
        });

        report.insert(
            format!("{}/{}", resource.project_namespace, resource.name),
            (true, exist),
        );
    }

    for resource in resources_cluster {
        let namespace = resource
            .metadata
            .namespace
            .as_ref()
            .unwrap()
            .replace("prj-", "");
        let name = resource.name_any();

        let exist = resources_state
            .iter()
            .any(|r| r.project_namespace.eq(&namespace) && r.name.eq(&name));

        report
            .entry(format!("{}/{}", namespace, name))
            .and_modify(|r| r.1 = exist)
            .or_insert((exist, true));
    }

    let report: Vec<(String, (bool, bool))> = report
        .into_iter()
        .filter(|(_, (in_state, in_cluster))| !(*in_state && *in_cluster))
        .collect();

    let mut table = Table::new();
    table.set_header(vec!["", "port", "state", "cluster"]);

    for (index, (resource_key, (state_exists, cluster_exists))) in report.iter().enumerate() {
        table.add_row(vec![
            &(index + 1).to_string(),
            &resource_key,
            &state_exists.to_string(),
            &cluster_exists.to_string(),
        ]);
    }

    match output {
        OutputFormat::Table => output_table_diff(report),
        OutputFormat::Json => todo!("not implemented"),
        OutputFormat::Csv => output_csv_diff(report),
    };

    Ok(())
}

fn output_csv_usage(report: Vec<UsageReport>, period: &str) {
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

fn output_json_usage(report: Vec<UsageReport>) {
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

fn output_table_usage(report: Vec<UsageReport>) {
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

fn output_table_project(projects: Vec<ProjectTable>) {
    let mut table = Table::new();
    table.set_header(vec![
        "",
        "name",
        "namespace",
        "stripe ID",
        "status",
        "email",
        "createdAt",
    ]);

    for (i, p) in projects.iter().enumerate() {
        table.add_row(vec![
            &(i + 1).to_string(),
            &p.name,
            &p.namespace,
            &p.billing_provider_id,
            &p.status.to_string(),
            &p.email,
            &p.created_at.to_rfc3339(),
        ]);
    }

    println!("{table}");
}

fn output_table_diff(report: Vec<(String, (bool, bool))>) {
    let mut table = Table::new();
    table.set_header(vec!["", "port", "state", "cluster"]);

    for (index, (resource_key, (state_exists, cluster_exists))) in report.iter().enumerate() {
        table.add_row(vec![
            &(index + 1).to_string(),
            &resource_key,
            &state_exists.to_string(),
            &cluster_exists.to_string(),
        ]);
    }

    println!("{table}");
}

fn output_csv_diff(report: Vec<(String, (bool, bool))>) {
    let path = "diff.csv";
    let result = csv::Writer::from_path(path);
    if let Err(error) = result {
        error!(?error);
        return;
    }

    let mut wtr = result.unwrap();

    let result = wtr.write_record(["", "project", "in_state", "in_cluster"]);
    if let Err(error) = result {
        error!(?error);
        return;
    }

    for (index, (resource_key, (state_exists, cluster_exists))) in report.iter().enumerate() {
        let result = wtr.write_record([
            &(index + 1).to_string(),
            resource_key,
            &state_exists.to_string(),
            &cluster_exists.to_string(),
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

pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

struct ProjectTable {
    pub name: String,
    pub namespace: String,
    pub email: String,
    pub status: ProjectStatus,
    pub billing_provider_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct BackofficeTlsConfig {
    pub ssl_crt_path: PathBuf,
    pub ssl_key_path: PathBuf,
}

#[derive(Debug)]
pub struct BackofficeConfig {
    pub db_path: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
    pub tls_config: Option<BackofficeTlsConfig>,

    pub auth_url: String,
    pub auth_client_id: String,
    pub auth_client_secret: String,
    pub auth_audience: String,
}
