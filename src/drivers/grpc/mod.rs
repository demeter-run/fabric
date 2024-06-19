use anyhow::Result;
use std::net::SocketAddr;
use std::str::FromStr;
use std::{path::Path, sync::Arc};
use tonic::transport::Server;

use dmtri::demeter::ops::v1alpha::project_service_server::ProjectServiceServer;

use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};
use crate::driven::kafka::KafkaEventBridge;

mod account;
mod project;

pub async fn server() -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new("dev.db")).await?);
    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let event_bridge = Arc::new(KafkaEventBridge::new(&["localhost:9092".into()], "events")?);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(dmtri::demeter::ops::v1alpha::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(protoc_wkt::google::protobuf::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let project_inner = project::ProjectServiceImpl::new(project_cache, event_bridge);
    let project_service = ProjectServiceServer::new(project_inner);

    let address = SocketAddr::from_str("0.0.0.0:5000")?;

    Server::builder()
        .add_service(reflection)
        .add_service(project_service)
        .serve(address)
        .await?;

    Ok(())
}
