use anyhow::Result;
use chrono::Utc;
use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tracing::{error, info};

use crate::{
    domain::provisional::command::sweep_expired,
    driven::{
        cache::{project::SqliteProjectDrivenCache, SqliteCache},
        kafka::KafkaProducer,
    },
};

/// Expires unclaimed provisional projects on a fixed interval, for as long as the rpc runs, with
/// registration on or off.
///
/// The first sweep waits a full interval. A restarted rpc first catches its projection up on the
/// events it missed, and a sweep over a projection that has not caught up could expire a project
/// whose claim it has not seen yet. A failed sweep is logged and retried on the next tick, so it
/// never stops the rpc.
pub async fn sweep(config: SweepConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache));
    let event_bridge = Arc::new(KafkaProducer::new(&config.topic, &config.kafka)?);

    let mut ticker = interval_at(Instant::now() + config.interval, config.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    info!(interval = ?config.interval, "provisional expiry sweep running");
    loop {
        ticker.tick().await;

        match sweep_expired(project_cache.clone(), event_bridge.clone(), Utc::now()).await {
            Ok(0) => {}
            Ok(expired) => info!(expired, "provisional projects expired"),
            Err(err) => error!(error = err.to_string(), "provisional expiry sweep failed"),
        }
    }
}

pub struct SweepConfig {
    pub db_path: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
    pub interval: Duration,
}
