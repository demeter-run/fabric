use anyhow::Result;
use dmtri::demeter::ops::v1alpha::port_service_server::PortServiceServer;
use std::net::SocketAddr;
use std::str::FromStr;
use std::{path::Path, sync::Arc};
use tonic::transport::Server;
use tracing::info;

use dmtri::demeter::ops::v1alpha::project_service_server::ProjectServiceServer;

use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};
use crate::driven::kafka::KafkaProducer;

mod port;
mod project;
mod user;

pub async fn server(addr: &str, db_path: &str, brokers: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&db_path)).await?);
    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let event_bridge = Arc::new(KafkaProducer::new(brokers, "events")?);

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

    let address = SocketAddr::from_str(addr)?;

    info!(address = addr, "Server running");

    Server::builder()
        .add_service(reflection)
        .add_service(project_service)
        .add_service(port_service)
        .serve(address)
        .await?;

    Ok(())
}
