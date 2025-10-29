use anyhow::{bail, Result};
use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use comfy_table::Table;
use futures::future::try_join_all;
use include_dir::{include_dir, Dir};
use kube::ResourceExt;
use serde_json::json;
use uuid::Uuid;
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{
        DEFAULT_CATEGORY, auth::{Auth0Driven, Auth0Profile}, event::{
            EventDrivenBridge, ProjectDeleted, ProjectUpdated, ResourceCreated, ResourceDeleted, ResourceUpdated
        }, metadata::{KnownField, MetadataDriven}, project::{
            ProjectStatus, ProjectUserProject, cache::{ProjectDrivenCache, ProjectDrivenCacheBackoffice}
        }, resource::{
            ResourceStatus, cache::{ResourceDrivenCache, ResourceDrivenCacheBackoffice}, cluster::ResourceDrivenClusterBackoffice, command::{build_key, encode_key}
        }, usage::{UsageReport, UsageReportImpl, cache::UsageDrivenCacheBackoffice}, utils::{self, get_schema_from_crd}
    },
    driven::{
        auth0::Auth0DrivenImpl,
        cache::{
            SqliteCache, project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache, usage::SqliteUsageDrivenCache
        },
        k8s::K8sCluster,
        kafka::KafkaProducer,
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

    let clusters = usage_cache.find_clusters(period).await?;

    for cluster in clusters {
        let report = usage_cache
            .find_report_aggregated(period, &cluster)
            .await?
            .calculate_cost(metadata.clone(), true);

        match output {
            OutputFormat::Table => output_table_usage(report, &cluster),
            OutputFormat::Json => output_json_usage(report, &cluster),
            OutputFormat::Csv => output_csv_usage(report, &cluster, period),
        };
    }

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
                    id: p.id,
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
            id: p.id,
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

pub async fn rename_project(
    config: BackofficeConfig,
    id: String,
    new_name: String,
    dry_run: bool,
) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let cache: Box<dyn ProjectDrivenCache> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    let event = Arc::new(KafkaProducer::new(
        &config.topic_events,
        &config.kafka_producer,
    )?);

    if cache.find_by_id(&id).await?.is_none() {
        error!("Failed to locate project");
        return Ok(());
    };

    let evt = ProjectUpdated {
        id: id.clone(),
        name: Some(new_name),
        status: None,
        updated_at: Utc::now(),
    };

    if dry_run {
        info!("event to dispath: {:?}", evt)
    } else {
        event.dispatch(evt.into()).await?;
        info!(project = &id, "project updated");
    }

    Ok(())
}

pub async fn delete_project(config: BackofficeConfig, id: String, dry_run: bool) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let cache: Box<dyn ProjectDrivenCache> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    let event = Arc::new(KafkaProducer::new(
        &config.topic_events,
        &config.kafka_producer,
    )?);

    let project = match cache.find_by_id(&id).await? {
        Some(project) => project,
        None => {
            error!("Failed to locate project");
            return Ok(());
        }
    };

    let evt = ProjectDeleted {
        id: id.clone(),
        namespace: project.namespace,
        deleted_at: Utc::now(),
    };

    if dry_run {
        info!("event to dispath: {:?}", evt)
    } else {
        event.dispatch(evt.into()).await?;
        info!(project = &id, "project deleted");
    }

    Ok(())
}

pub async fn create_resource(
    config: BackofficeConfig,
    project_id: String,
    kind: String,
    spec: String,
    dry_run: bool,
) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let resource_cache: Box<dyn ResourceDrivenCache> = Box::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));
    let project_cache: Box<dyn ProjectDrivenCache> = Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let metadata = Box::new(FileMetadata::new(&config.crds_path)?);

    let event = Arc::new(KafkaProducer::new(
        &config.topic_events,
        &config.kafka_producer,
    )?);

    let name = format!(
        "{}-{}",
        kind.to_lowercase().replace("port", ""),
        utils::get_random_salt()
    );

    if resource_cache
        .find_by_name(&project_id, &name)
        .await?
        .is_some()
    {
        error!("Invalid random name, try again");
        return Ok(());
    }

    let Some(metadata) = metadata.find_by_kind(&kind)? else {
        error!("Kind not supported");
        return Ok(());
    };

    let project = match project_cache.find_by_id(&project_id).await? {
        Some(project) => project,
        None => {
            error!("Failed to locate project");
            return Ok(());
        }
    };
    let resource_id = Uuid::new_v4().to_string();

    let mut spec_json = match serde_json::from_str(&spec)? {
        serde_json::Value::Object(v) => v,
        _ => {
            error!("invalid spec json");
            return Ok(());
        }
    };

    if let Some(status_schema) = get_schema_from_crd(&metadata.crd, "status") {
        for (key, _) in status_schema {
            if let Ok(status_field) = key.parse::<KnownField>() {
                let value = match status_field {
                    KnownField::AuthToken => {
                        let key = build_key(&project.id, &resource_id)?;
                        encode_key(key, &kind)?
                    }
                    KnownField::Username => {
                        let user_key = build_key(&project.id, &resource_id)?;
                        encode_key(user_key, &kind)?
                    }
                    KnownField::Password => {
                        let password_key = build_key(&project.id, &resource_id)?;
                        BASE64_STANDARD_NO_PAD.encode(password_key)
                    }
                };
                spec_json.insert(key, serde_json::Value::String(value));
            }
        }
    };

    let evt = ResourceCreated {
        id: resource_id.clone(),
        project_id: project.id,
        project_namespace: project.namespace,
        name,
        kind: kind.clone(),
        category: metadata
            .crd
            .spec
            .names
            .categories
            .and_then(|c| c.first().map(String::to_owned))
            .unwrap_or(DEFAULT_CATEGORY.to_string()),
        spec: serde_json::to_string(&spec_json)?,
        status: ResourceStatus::Active.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    if dry_run {
        info!("event to dispath: {:?}", evt)
    } else {
        event.dispatch(evt.into()).await?;
        info!(resource_id, "resource created");
    }
    Ok(())
}

pub async fn delete_resource(
    config: BackofficeConfig,
    id: String,
    project_id: String,
    dry_run: bool,
) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache: Box<dyn ProjectDrivenCache> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    let resource_cache: Box<dyn ResourceDrivenCache> =
        Box::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

    let event = Arc::new(KafkaProducer::new(
        &config.topic_events,
        &config.kafka_producer,
    )?);

    let resource = match resource_cache.find_by_id(&id).await? {
        Some(resource) => resource,
        None => {
            error!("Failed to locate resource");
            return Ok(());
        }
    };

    let project = match project_cache.find_by_id(&project_id).await? {
        Some(project) => project,
        None => {
            error!("Failed to locate project");
            return Ok(());
        }
    };

    let evt = ResourceDeleted {
        id,
        project_id: project.id,
        project_namespace: project.namespace,
        name: resource.name,
        kind: resource.kind.clone(),
        status: ResourceStatus::Deleted.to_string(),
        deleted_at: Utc::now(),
    };

    if dry_run {
        info!("event to dispath: {:?}", evt)
    } else {
        event.dispatch(evt.into()).await?;
        info!(resource = resource.kind, "resource deleted");
    }

    Ok(())
}

pub async fn patch_resource(
    config: BackofficeConfig,
    id: String,
    project_id: String,
    patch: String,
    dry_run: bool,
) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache: Box<dyn ProjectDrivenCache> =
        Box::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));

    let resource_cache: Box<dyn ResourceDrivenCache> =
        Box::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

    let event = Arc::new(KafkaProducer::new(
        &config.topic_events,
        &config.kafka_producer,
    )?);

    let resource = match resource_cache.find_by_id(&id).await? {
        Some(resource) => resource,
        None => {
            error!("Failed to locate resource");
            return Ok(());
        }
    };

    let project = match project_cache.find_by_id(&project_id).await? {
        Some(project) => project,
        None => {
            error!("Failed to locate project");
            return Ok(());
        }
    };

    if resource.project_id != project_id {
        error!("Resource doesn't match project.");
        return Ok(());
    }

    if let Err(err) = serde_json::from_str::<serde_json::Value>(&patch) {
        error!(err = err.to_string(), "Failed to parse patch as json");
        return Ok(());
    }

    let evt = ResourceUpdated {
        id,
        project_id: project.id,
        project_namespace: project.namespace,
        name: resource.name,
        kind: resource.kind.clone(),
        spec_patch: patch,
        updated_at: Utc::now(),
    };

    if dry_run {
        info!("event to dispath: {:?}", evt)
    } else {
        event.dispatch(evt.into()).await?;
        info!(resource = resource.kind, "resource patched");
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
        "id",
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
            &r.id,
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

pub async fn fetch_new_users(
    config: BackofficeConfig,
    after: &str,
    output: OutputFormat,
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

    let project_users = project_cache.find_new_users(after).await?;
    if project_users.is_empty() {
        bail!("No one new user was found")
    }

    let tasks = project_users
        .chunks(60)
        .map(|chunk| async {
            let ids = chunk
                .to_vec()
                .iter()
                .map(|p| format!("user_id:{}", p.user_id.clone()))
                .collect::<Vec<String>>()
                .join(" OR ");
            auth0.find_info(ids.as_ref()).await
        })
        .collect::<Vec<_>>();

    let profiles = try_join_all(tasks).await?;
    let profiles = profiles.into_iter().flatten().collect::<Vec<_>>();

    match output {
        OutputFormat::Table => output_table_new_users(project_users, profiles),
        OutputFormat::Json => todo!("not implemented"),
        OutputFormat::Csv => output_csv_new_users(project_users, profiles),
    };

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
            .entry(format!("{namespace}/{name}"))
            .and_modify(|r| r.1 = exist)
            .or_insert((exist, true));
    }

    let report: Vec<(String, (bool, bool))> = report
        .into_iter()
        .filter(|(_, (in_state, in_cluster))| !(*in_state && *in_cluster))
        .collect();

    match output {
        OutputFormat::Table => output_table_diff(report),
        OutputFormat::Json => todo!("not implemented"),
        OutputFormat::Csv => output_csv_diff(report),
    };

    Ok(())
}

fn output_csv_usage(report: Vec<UsageReport>, cluster_id: &str, period: &str) {
    let path = format!("{cluster_id}.{period}.csv");
    let result = csv::Writer::from_path(&path);
    if let Err(error) = result {
        error!(?error);
        return;
    }

    let mut wtr = result.unwrap();

    let result = wtr.write_record([
        "",
        "cluster",
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
            &r.cluster_id,
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

    info!("File {} created", path)
}

fn output_json_usage(report: Vec<UsageReport>, cluster_id: &str) {
    let mut json = vec![];

    for r in report {
        json.push(json!({
            "cluster_id": cluster_id,
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

fn output_table_usage(report: Vec<UsageReport>, cluster_id: &str) {
    let mut table = Table::new();
    table.set_header(vec![
        "",
        "cluster",
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
            cluster_id,
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
        "id",
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
            &p.id,
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

    println!("File {path} created")
}

fn output_table_new_users(project_users: Vec<ProjectUserProject>, profiles: Vec<Auth0Profile>) {
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
}

fn output_csv_new_users(project_users: Vec<ProjectUserProject>, profiles: Vec<Auth0Profile>) {
    let path = "new-users.csv";
    let result = csv::Writer::from_path(path);
    if let Err(error) = result {
        error!(?error);
        return;
    }

    let mut wtr = result.unwrap();

    let result = wtr.write_record([
        "",
        "id",
        "name",
        "email",
        "role",
        "project",
        "stripe ID",
        "createdAt",
    ]);

    if let Err(error) = result {
        error!(?error);
        return;
    }

    for (i, u) in project_users.iter().enumerate() {
        let (name, email) = match profiles.iter().find(|a| a.user_id == u.user_id) {
            Some(a) => (a.name.clone(), a.email.clone()),
            None => ("unknown".into(), "unknown".into()),
        };

        let result = wtr.write_record(vec![
            &(i + 1).to_string(),
            &u.user_id,
            &name,
            &email,
            &u.role.to_string(),
            &u.project_namespace,
            &u.project_billing_provider_id,
            &u.created_at.to_rfc3339(),
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

    println!("File {path} created")
}

pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

struct ProjectTable {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub email: String,
    pub status: ProjectStatus,
    pub billing_provider_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct BackofficeConfig {
    pub db_path: String,
    pub crds_path: PathBuf,

    pub auth_url: String,
    pub auth_client_id: String,
    pub auth_client_secret: String,
    pub auth_audience: String,

    pub topic_events: String,
    pub kafka_producer: HashMap<String, String>,
}
