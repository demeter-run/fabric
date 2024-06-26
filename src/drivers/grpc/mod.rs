use anyhow::Result;
use std::net::SocketAddr;
use std::str::FromStr;
use std::{path::Path, sync::Arc};
use tonic::transport::Server;
use tracing::info;

use dmtri::demeter::ops::v1alpha::project_service_server::ProjectServiceServer;

use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};
use crate::driven::kafka::KafkaEventBridge;

mod account;
mod project;

pub async fn server(addr: &str, db_path: &str, kafka_host: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&db_path)).await?);
    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let event_bridge = Arc::new(KafkaEventBridge::new(&[kafka_host.into()], "events")?);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(dmtri::demeter::ops::v1alpha::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let project_inner = project::ProjectServiceImpl::new(project_cache, event_bridge);
    let project_service = ProjectServiceServer::new(project_inner);

    let address = SocketAddr::from_str(addr)?;

    info!(address = addr, "Server running");

    Server::builder()
        .add_service(reflection)
        .add_service(project_service)
        .serve(address)
        .await?;

    Ok(())
}
