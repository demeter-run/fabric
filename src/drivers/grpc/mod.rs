use anyhow::Result;
use dmtri::demeter::ops::v1alpha::port_service_server::PortServiceServer;
use dmtri::demeter::ops::v1alpha::user_service_server::UserServiceServer;
use std::net::SocketAddr;
use std::str::FromStr;
use std::{path::Path, sync::Arc};
use tonic::transport::Server;
use tracing::info;

use dmtri::demeter::ops::v1alpha::project_service_server::ProjectServiceServer;

use crate::driven::auth0::Auth0Provider;
use crate::driven::cache::user::SqliteUserCache;
use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};
use crate::driven::kafka::KafkaProducer;

//mod middlewares;
mod port;
mod project;
mod user;

pub async fn server(config: GrpcConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache.clone()));
    let user_cache = Arc::new(SqliteUserCache::new(sqlite_cache.clone()));

    let event_bridge = Arc::new(KafkaProducer::new(&config.brokers, "events")?);

    let auth_provider = Arc::new(Auth0Provider::new(&config.auth_url));

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(dmtri::demeter::ops::v1alpha::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let project_inner =
        project::ProjectServiceImpl::new(project_cache.clone(), event_bridge.clone());
    let project_service = ProjectServiceServer::new(project_inner);

    let port_inner = port::PortServiceImpl::new(project_cache.clone(), event_bridge.clone());
    let port_service = PortServiceServer::new(port_inner);

    let user_inner = user::UserServiceImpl::new(
        user_cache.clone(),
        auth_provider.clone(),
        event_bridge.clone(),
    );
    let user_service = UserServiceServer::new(user_inner);

    let address = SocketAddr::from_str(&config.addr)?;

    info!(address = config.addr, "Server running");

    Server::builder()
        .add_service(reflection)
        .add_service(project_service)
        .add_service(port_service)
        .add_service(user_service)
        .serve(address)
        .await?;

    Ok(())
}

pub struct GrpcConfig {
    pub addr: String,
    pub db_path: String,
    pub brokers: String,
    pub auth_url: String,
}
