use anyhow::Result;
use dmtri::demeter::ops::v1alpha::key_value_service_server::KeyValueServiceServer;
use dmtri::demeter::ops::v1alpha::logs_service_server::LogsServiceServer;
use dmtri::demeter::ops::v1alpha::metadata_service_server::MetadataServiceServer;
use dmtri::demeter::ops::v1alpha::resource_service_server::ResourceServiceServer;
use dmtri::demeter::ops::v1alpha::signer_service_server::SignerServiceServer;
use dmtri::demeter::ops::v1alpha::usage_service_server::UsageServiceServer;
use middlewares::auth::AuthenticatorImpl;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::{path::Path, sync::Arc};
use tonic::{
    transport::{Identity, Server, ServerTlsConfig},
    Status,
};
use tower_http::cors::CorsLayer;
use tracing::{error, info};

use dmtri::demeter::ops::v1alpha::project_service_server::ProjectServiceServer;

use crate::domain::error::Error;
use crate::driven::auth0::Auth0DrivenImpl;
use crate::driven::cache::project::SqliteProjectDrivenCache;
use crate::driven::cache::resource::SqliteResourceDrivenCache;
use crate::driven::cache::usage::SqliteUsageDrivenCache;
use crate::driven::cache::SqliteCache;
use crate::driven::kafka::KafkaProducer;
use crate::driven::metadata::FileMetadata;
use crate::driven::prometheus::metrics::MetricsDriven;
use crate::driven::ses::SESDrivenImpl;
use crate::driven::stripe::StripeDrivenImpl;
use crate::driven::worker::signer::VaultWorkerSignerDrivenStorage;
use crate::driven::worker::storage::keyvalue::PostgresWorkerKeyValueDrivenStorage;
use crate::driven::worker::storage::logs::PostgresWorkerLogsDrivenStorage;
use crate::driven::worker::storage::PostgresStorage;

mod metadata;
mod middlewares;
mod project;
mod resource;
mod usage;
mod worker;

pub async fn server(config: GrpcConfig, metrics: Arc<MetricsDriven>) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let resource_cache = Arc::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));
    let usage_cache = Arc::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    let event_bridge = Arc::new(KafkaProducer::new(&config.topic, &config.kafka)?);

    let metadata = Arc::new(FileMetadata::new(&config.crds_path)?);

    let auth0 = Arc::new(
        Auth0DrivenImpl::try_new(
            &config.auth_url,
            &config.auth_client_id,
            &config.auth_client_secret,
            &config.auth_audience,
        )
        .await?,
    );
    let stripe = Arc::new(StripeDrivenImpl::new(
        &config.stripe_url,
        &config.stripe_api_key,
    ));
    let email = Arc::new(SESDrivenImpl::new(
        &config.ses_access_key_id,
        &config.ses_secret_access_key,
        &config.ses_region,
        &config.ses_verified_email,
    ));

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(dmtri::demeter::ops::v1alpha::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
        .build_v1alpha()
        .unwrap();

    let auth_interceptor =
        AuthenticatorImpl::new(auth0.clone(), project_cache.clone(), metrics.clone());

    let project_inner = project::ProjectServiceImpl::new(
        project_cache.clone(),
        event_bridge.clone(),
        auth0.clone(),
        stripe.clone(),
        email.clone(),
        metrics.clone(),
        config.secret.clone(),
        config.invite_ttl,
    );
    let project_service =
        ProjectServiceServer::with_interceptor(project_inner, auth_interceptor.clone());
    let project_service = tonic_web::enable(project_service);

    let resource_inner = resource::ResourceServiceImpl::new(
        project_cache.clone(),
        resource_cache.clone(),
        event_bridge.clone(),
        metadata.clone(),
        metrics.clone(),
    );
    let resource_service =
        ResourceServiceServer::with_interceptor(resource_inner, auth_interceptor.clone());
    let resource_service = tonic_web::enable(resource_service);

    let metadata_inner = metadata::MetadataServiceImpl::new(metadata.clone(), metrics.clone());
    let metadata_service = MetadataServiceServer::new(metadata_inner);
    let metadata_service = tonic_web::enable(metadata_service);

    let usage_inner = usage::UsageServiceImpl::new(
        project_cache.clone(),
        usage_cache.clone(),
        metadata.clone(),
        metrics.clone(),
    );
    let usage_service = UsageServiceServer::with_interceptor(usage_inner, auth_interceptor.clone());
    let usage_service = tonic_web::enable(usage_service);

    let (worker_kv_service, worker_logs_service) = if let Some(pg_url) = config.balius_pg_url {
        let storage = Arc::new(PostgresStorage::new(&pg_url).await?);
        let kv_storage = Arc::new(PostgresWorkerKeyValueDrivenStorage::new(storage.clone()));
        let logs_storage = Arc::new(PostgresWorkerLogsDrivenStorage::new(storage.clone()));

        let kv_inner = worker::WorkerKeyValueServiceImpl::new(
            project_cache.clone(),
            resource_cache.clone(),
            kv_storage.clone(),
            metrics.clone(),
        );
        let kv_service =
            KeyValueServiceServer::with_interceptor(kv_inner, auth_interceptor.clone());

        let logs_inner = worker::WorkerLogsServiceImpl::new(
            project_cache.clone(),
            resource_cache.clone(),
            logs_storage.clone(),
            metrics.clone(),
        );
        let logs_service =
            LogsServiceServer::with_interceptor(logs_inner, auth_interceptor.clone());

        (
            Some(tonic_web::enable(kv_service)),
            Some(tonic_web::enable(logs_service)),
        )
    } else {
        (None, None)
    };

    let worker_signer_service = if let (Some(address), Some(token)) =
        (config.balius_vault_address, config.balius_vault_token)
    {
        let storage = Arc::new(VaultWorkerSignerDrivenStorage::try_new(&address, &token)?);
        let signer_inner = worker::WorkerSignerServiceImpl::new(
            project_cache.clone(),
            resource_cache.clone(),
            storage.clone(),
            metrics.clone(),
        );
        let signer_service =
            SignerServiceServer::with_interceptor(signer_inner, auth_interceptor.clone());

        Some(tonic_web::enable(signer_service))
    } else {
        None
    };

    let address = SocketAddr::from_str(&config.addr)?;

    let mut server = Server::builder()
        .accept_http1(true)
        .layer(CorsLayer::permissive());

    if let Some(tls) = config.tls_config {
        let cert = std::fs::read_to_string(tls.ssl_crt_path)?;
        let key = std::fs::read_to_string(tls.ssl_key_path)?;
        let identity = Identity::from_pem(cert, key);

        server = server.tls_config(ServerTlsConfig::new().identity(identity))?;
    }

    info!(address = config.addr, "GRPC server running");

    server
        .add_service(project_service)
        .add_service(resource_service)
        .add_service(usage_service)
        .add_service(metadata_service)
        .add_service(reflection)
        .add_optional_service(worker_kv_service)
        .add_optional_service(worker_logs_service)
        .add_optional_service(worker_signer_service)
        .serve(address)
        .await?;

    Ok(())
}

pub struct GrpcTlsConfig {
    pub ssl_crt_path: PathBuf,
    pub ssl_key_path: PathBuf,
}

pub struct GrpcConfig {
    pub addr: String,
    pub db_path: String,
    pub crds_path: PathBuf,
    pub auth_url: String,
    pub auth_client_id: String,
    pub auth_client_secret: String,
    pub auth_audience: String,
    pub stripe_url: String,
    pub stripe_api_key: String,
    pub secret: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
    pub invite_ttl: Duration,
    pub ses_access_key_id: String,
    pub ses_secret_access_key: String,
    pub ses_region: String,
    pub ses_verified_email: String,
    pub tls_config: Option<GrpcTlsConfig>,
    pub balius_pg_url: Option<String>,
    pub balius_vault_token: Option<String>,
    pub balius_vault_address: Option<String>,
}

impl From<Error> for Status {
    fn from(value: Error) -> Self {
        match value {
            Error::Unauthorized(err) => Status::permission_denied(err),
            Error::CommandMalformed(err) => Status::failed_precondition(err),
            Error::SecretExceeded(err) => Status::resource_exhausted(err),
            Error::Unexpected(err) => {
                error!(err);
                Status::internal("internal error")
            }
        }
    }
}

fn handle_error_metric(metrics: Arc<MetricsDriven>, domain: &str, error: &Error) {
    if let Error::Unexpected(err) = error {
        metrics.domain_error("grpc", domain, &err.to_string());
    }
}
