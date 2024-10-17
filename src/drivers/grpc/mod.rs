use anyhow::Result;
use dmtri::demeter::ops::v1alpha::metadata_service_server::MetadataServiceServer;
use dmtri::demeter::ops::v1alpha::resource_service_server::ResourceServiceServer;
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
use crate::driven::ses::SESDrivenImpl;
use crate::driven::stripe::StripeDrivenImpl;

mod metadata;
mod middlewares;
mod project;
mod resource;
mod usage;

pub async fn server(config: GrpcConfig) -> Result<()> {
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
        .build()
        .unwrap();

    let auth_interceptor = AuthenticatorImpl::new(auth0.clone(), project_cache.clone());

    let project_inner = project::ProjectServiceImpl::new(
        project_cache.clone(),
        event_bridge.clone(),
        auth0.clone(),
        stripe.clone(),
        email.clone(),
        config.secret.clone(),
        config.invite_ttl,
    );
    let project_service =
        ProjectServiceServer::with_interceptor(project_inner, auth_interceptor.clone());

    let resource_inner = resource::ResourceServiceImpl::new(
        project_cache.clone(),
        resource_cache.clone(),
        event_bridge.clone(),
        metadata.clone(),
    );
    let resource_service =
        ResourceServiceServer::with_interceptor(resource_inner, auth_interceptor.clone());

    let metadata_inner = metadata::MetadataServiceImpl::new(metadata.clone());
    let metadata_service = MetadataServiceServer::new(metadata_inner);

    let usage_inner =
        usage::UsageServiceImpl::new(project_cache.clone(), usage_cache.clone(), metadata.clone());
    let usage_service = UsageServiceServer::with_interceptor(usage_inner, auth_interceptor.clone());

    let address = SocketAddr::from_str(&config.addr)?;

    let mut server = if let Some(tls) = config.tls_config {
        let cert = std::fs::read_to_string(tls.ssl_crt_path)?;
        let key = std::fs::read_to_string(tls.ssl_key_path)?;
        let identity = Identity::from_pem(cert, key);

        Server::builder().tls_config(ServerTlsConfig::new().identity(identity))?
    } else {
        Server::builder()
    };

    info!(address = config.addr, "Server running");
    server
        .add_service(reflection)
        .add_service(project_service)
        .add_service(resource_service)
        .add_service(metadata_service)
        .add_service(usage_service)
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
