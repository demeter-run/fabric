use anyhow::Result;
use dmtri::demeter::ops::v1alpha::metadata_service_server::MetadataServiceServer;
use dmtri::demeter::ops::v1alpha::resource_service_server::ResourceServiceServer;
use middlewares::auth::AuthenticatorImpl;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::{path::Path, sync::Arc};
use tonic::transport::Server;
use tonic::Status;
use tracing::{error, info};

use dmtri::demeter::ops::v1alpha::project_service_server::ProjectServiceServer;

use crate::domain::error::Error;
use crate::driven::auth::Auth0Provider;
use crate::driven::cache::project::SqliteProjectDrivenCache;
use crate::driven::cache::resource::SqliteResourceDrivenCache;
use crate::driven::cache::SqliteCache;
use crate::driven::kafka::KafkaProducer;
use crate::driven::metadata::MetadataCrd;

mod metadata;
mod middlewares;
mod project;
mod resource;

pub async fn server(config: GrpcConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let resource_cache = Arc::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

    let event_bridge = Arc::new(KafkaProducer::new(&config.topic, &config.kafka)?);

    let metadata = Arc::new(MetadataCrd::new(&config.crds_path)?);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(dmtri::demeter::ops::v1alpha::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let auth = AuthenticatorImpl::new(
        Arc::new(Auth0Provider::try_new(&config.auth_url).await?),
        project_cache.clone(),
    );

    let project_inner = project::ProjectServiceImpl::new(
        project_cache.clone(),
        event_bridge.clone(),
        config.secret.clone(),
    );
    let project_service = ProjectServiceServer::with_interceptor(project_inner, auth.clone());

    let resource_inner = resource::ResourceServiceImpl::new(
        project_cache.clone(),
        resource_cache.clone(),
        event_bridge.clone(),
        metadata.clone(),
    );
    let resource_service = ResourceServiceServer::with_interceptor(resource_inner, auth.clone());

    let metadata_inner = metadata::MetadataServiceImpl::new(metadata.clone());
    let metadata_service = MetadataServiceServer::new(metadata_inner);

    let address = SocketAddr::from_str(&config.addr)?;

    info!(address = config.addr, "Server running");

    Server::builder()
        .add_service(reflection)
        .add_service(project_service)
        .add_service(resource_service)
        .add_service(metadata_service)
        .serve(address)
        .await?;

    Ok(())
}

pub struct GrpcConfig {
    pub addr: String,
    pub db_path: String,
    pub crds_path: PathBuf,
    pub auth_url: String,
    pub secret: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
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
