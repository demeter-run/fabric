use anyhow::Result;
use std::net::SocketAddr;
use std::str::FromStr;
use std::{path::Path, sync::Arc};
use tonic::transport::Server;

use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};
use crate::driven::kafka::KafkaEventBridge;

mod project;

pub mod proto {
    pub mod project {
        tonic::include_proto!("fabric.project.v1alpha");
    }
}

pub async fn server() -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new("dev.db")).await?);
    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let event_bridge = Arc::new(KafkaEventBridge::new(&["localhost:9092".into()], "events")?);

    let project_inner = project::ProjectServiceImpl::new(project_cache, event_bridge);
    let project_service =
        proto::project::project_service_server::ProjectServiceServer::new(project_inner);

    let address = SocketAddr::from_str("0.0.0.0:5000")?;

    Server::builder()
        .add_service(project_service)
        .serve(address)
        .await?;

    Ok(())
}
