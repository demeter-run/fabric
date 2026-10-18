use anyhow::Result;
use chrono::{DateTime, Utc};
use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tracing::{error, info};

use crate::{
    domain::{
        event::EventDrivenBridge, project::cache::ProjectDrivenCache,
        provisional::command::sweep_expired,
    },
    driven::{
        cache::{project::SqliteProjectDrivenCache, SqliteCache},
        kafka::KafkaProducer,
        prometheus::metrics::MetricsDriven,
    },
};

/// Expires unclaimed provisional projects on a fixed interval, for as long as the rpc runs, with
/// registration on or off.
///
/// The first sweep waits a full interval. A restarted rpc first catches its projection up on the
/// events it missed, and a sweep over a projection that has not caught up could expire a project
/// whose claim it has not seen yet. A failed sweep is logged and retried on the next tick, so it
/// never stops the rpc.
pub async fn sweep(config: SweepConfig, metrics: Arc<MetricsDriven>) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache));
    let event_bridge = Arc::new(KafkaProducer::new(&config.topic, &config.kafka)?);

    let mut ticker = interval_at(Instant::now() + config.interval, config.interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    info!(interval = ?config.interval, "provisional expiry sweep running");
    loop {
        ticker.tick().await;

        tick(
            project_cache.clone(),
            event_bridge.clone(),
            &metrics,
            Utc::now(),
            config.interval,
        )
        .await;
    }
}

/// Runs one sweep and records it: every tick counts as `ok` or `error`, and a successful one adds
/// what it expired and sets how many projects are overdue — still provisional more than one
/// interval past their expiry, which an earlier tick should already have expired.
async fn tick(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    metrics: &MetricsDriven,
    now: DateTime<Utc>,
    interval: Duration,
) {
    let result = async {
        let expired = sweep_expired(cache.clone(), event, now).await?;
        let overdue = cache
            .find_expired_provisional(&(now - interval))
            .await?
            .len();
        anyhow::Ok((expired, overdue))
    }
    .await;

    match result {
        Ok((expired, overdue)) => {
            metrics.provisional_sweep_ok(expired, overdue);
            if expired > 0 {
                info!(expired, "provisional projects expired");
            }
        }
        Err(err) => {
            metrics.provisional_sweep_error();
            error!(error = err.to_string(), "provisional expiry sweep failed");
        }
    }
}

pub struct SweepConfig {
    pub db_path: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
    pub interval: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        error::Error,
        event::MockEventDrivenBridge,
        project::{cache::MockProjectDrivenCache, Project, ProjectStatus},
    };

    const INTERVAL: Duration = Duration::from_secs(300);

    fn provisional(expires_at: DateTime<Utc>) -> Project {
        Project {
            status: ProjectStatus::Provisional,
            expires_at: Some(expires_at),
            ..Default::default()
        }
    }

    /// A cache holding `projects`, answering `find_expired_provisional` as the sqlite query does.
    fn cache(projects: Vec<Project>) -> MockProjectDrivenCache {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_expired_provisional()
            .returning(move |at| {
                Ok(projects
                    .iter()
                    .filter(|p| p.expires_at.is_some_and(|e| e < *at))
                    .cloned()
                    .collect())
            });
        cache
    }

    fn runs(metrics: &MetricsDriven, outcome: &str) -> u64 {
        metrics
            .provisional_sweep_runs
            .with_label_values(&[outcome])
            .get()
    }

    #[tokio::test]
    async fn it_should_count_a_tick_that_expires_a_project() {
        let now = Utc::now();
        let metrics = MetricsDriven::new().unwrap();
        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().times(1).returning(|_| Ok(()));

        let cache = cache(vec![provisional(now - chrono::Duration::seconds(10))]);
        tick(Arc::new(cache), Arc::new(event), &metrics, now, INTERVAL).await;

        assert_eq!(runs(&metrics, "ok"), 1);
        assert_eq!(runs(&metrics, "error"), 0);
        assert_eq!(metrics.provisional_projects_expired.get(), 1);
        assert_eq!(metrics.provisional_projects_overdue.get(), 0);
    }

    #[tokio::test]
    async fn it_should_count_a_tick_over_an_empty_cache() {
        let metrics = MetricsDriven::new().unwrap();
        let event = MockEventDrivenBridge::new();

        tick(
            Arc::new(cache(vec![])),
            Arc::new(event),
            &metrics,
            Utc::now(),
            INTERVAL,
        )
        .await;

        assert_eq!(runs(&metrics, "ok"), 1);
        assert_eq!(runs(&metrics, "error"), 0);
        assert_eq!(metrics.provisional_projects_expired.get(), 0);
        assert_eq!(metrics.provisional_projects_overdue.get(), 0);
    }

    #[tokio::test]
    async fn it_should_count_a_failing_tick_as_an_error() {
        let metrics = MetricsDriven::new().unwrap();
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_expired_provisional()
            .returning(|_| Err(Error::Unexpected("cache unavailable".into())));

        tick(
            Arc::new(cache),
            Arc::new(MockEventDrivenBridge::new()),
            &metrics,
            Utc::now(),
            INTERVAL,
        )
        .await;

        assert_eq!(runs(&metrics, "ok"), 0);
        assert_eq!(runs(&metrics, "error"), 1);
        assert_eq!(metrics.provisional_projects_expired.get(), 0);
    }

    #[tokio::test]
    async fn it_should_count_only_projects_past_one_interval_as_overdue() {
        let now = Utc::now();
        let interval = chrono::Duration::from_std(INTERVAL).unwrap();
        let metrics = MetricsDriven::new().unwrap();
        // The projection never applies the expiry, so both stay provisional and are re-expired.
        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().times(2).returning(|_| Ok(()));

        let cache = cache(vec![
            provisional(now - interval - chrono::Duration::seconds(1)),
            provisional(now - interval + chrono::Duration::seconds(1)),
        ]);
        tick(Arc::new(cache), Arc::new(event), &metrics, now, INTERVAL).await;

        assert_eq!(runs(&metrics, "ok"), 1);
        assert_eq!(metrics.provisional_projects_expired.get(), 2);
        assert_eq!(metrics.provisional_projects_overdue.get(), 1);
    }
}
