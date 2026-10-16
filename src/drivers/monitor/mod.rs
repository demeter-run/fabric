use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, collections::HashMap, sync::Arc};
use tracing::{error, info, warn};

use crate::{
    domain::{
        error::Error,
        event::Event,
        project::{self, cluster::ProjectDrivenCluster},
        resource::{self, cluster::ResourceDrivenCluster},
        Result as DomainResult,
    },
    driven::{k8s::K8sCluster, prometheus::metrics::MetricsDriven},
};

pub async fn subscribe(config: MonitorConfig, metrics: Arc<MetricsDriven>) -> Result<()> {
    let cluster = Arc::new(K8sCluster::new().await?);

    let mut client_config = ClientConfig::new();
    for (k, v) in config.kafka.iter() {
        client_config.set(k, v);
    }
    let consumer: StreamConsumer = client_config.create()?;
    consumer.subscribe(&[&config.topic])?;

    info!("Monitor subscribe running");
    loop {
        match consumer.recv().await {
            Err(error) => error!(?error, "kafka subscribe error"),
            Ok(message) => {
                let message = message.borrow();
                match message.try_into() {
                    Ok(event) => {
                        let result =
                            match apply_event(cluster.clone(), cluster.clone(), &event).await {
                                Some((domain, result)) => result.inspect_err(|err| {
                                    handle_error_metric(metrics.clone(), domain, err)
                                }),
                                None => {
                                    info!(event = event.key(), "bypass event");
                                    Ok(())
                                }
                            };

                        match result {
                            Ok(()) => {
                                info!(event = event.key(), "Successfully handled event")
                            }
                            Err(err) => warn!(
                                event = event.key(),
                                error = err.to_string(),
                                "Error running event."
                            ),
                        }

                        consumer.commit_message(message, CommitMode::Async)?;
                    }
                    Err(error) => {
                        error!(?error, "fail to convert message to event");
                        consumer.commit_message(message, CommitMode::Async)?;
                    }
                }
            }
        };
    }
}

/// Applies one event to the cluster: the domain it belongs to and the outcome, or `None` for an
/// event the daemon bypasses.
async fn apply_event(
    project_cluster: Arc<dyn ProjectDrivenCluster>,
    resource_cluster: Arc<dyn ResourceDrivenCluster>,
    event: &Event,
) -> Option<(&'static str, DomainResult<()>)> {
    match event {
        Event::ProjectCreated(evt) => Some((
            "project",
            project::cluster::apply_manifest(project_cluster, evt.clone()).await,
        )),
        Event::ProvisionalProjectCreated(evt) => Some((
            "project",
            project::cluster::apply_provisional_manifest(project_cluster, evt.clone()).await,
        )),
        Event::ProjectDeleted(evt) => Some((
            "project",
            project::cluster::delete_manifest(project_cluster, evt.clone()).await,
        )),
        Event::ProjectExpired(evt) => Some((
            "project",
            project::cluster::expire_manifest(project_cluster, evt.clone()).await,
        )),
        Event::ResourceCreated(evt) => Some((
            "resource",
            resource::cluster::apply_manifest(resource_cluster, evt.clone()).await,
        )),
        Event::ResourceUpdated(evt) => Some((
            "resource",
            resource::cluster::patch_manifest(resource_cluster, evt.clone()).await,
        )),
        Event::ResourceDeleted(evt) => Some((
            "resource",
            resource::cluster::delete_manifest(resource_cluster, evt.clone()).await,
        )),
        _ => None,
    }
}

#[derive(Debug)]
pub struct MonitorConfig {
    pub topic: String,
    pub kafka: HashMap<String, String>,
}

fn handle_error_metric(metrics: Arc<MetricsDriven>, domain: &str, error: &Error) {
    if let Error::Unexpected(err) = error {
        metrics.domain_error("monitor", domain, &err.to_string());
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Namespace;

    use super::*;
    use crate::domain::{
        event::{ProjectExpired, ProvisionalProjectCreated},
        project::cluster::MockProjectDrivenCluster,
        resource::cluster::MockResourceDrivenCluster,
    };

    fn named(namespace: &Namespace, name: &str) -> bool {
        namespace.metadata.name.as_deref() == Some(name)
    }

    #[tokio::test]
    async fn it_should_create_the_namespace_of_a_provisional_project() {
        let evt = ProvisionalProjectCreated::default();
        let expected = format!("prj-{}", evt.namespace);

        let mut project_cluster = MockProjectDrivenCluster::new();
        project_cluster
            .expect_create()
            .withf(move |namespace| named(namespace, &expected))
            .times(1)
            .return_once(|_| Ok(()));

        let result = apply_event(
            Arc::new(project_cluster),
            Arc::new(MockResourceDrivenCluster::new()),
            &evt.into(),
        )
        .await;

        assert!(matches!(result, Some(("project", Ok(())))));
    }

    #[tokio::test]
    async fn it_should_delete_the_namespace_of_an_expired_project() {
        let evt = ProjectExpired::default();
        let expected = format!("prj-{}", evt.namespace);

        let mut project_cluster = MockProjectDrivenCluster::new();
        project_cluster
            .expect_delete()
            .withf(move |namespace| named(namespace, &expected))
            .times(1)
            .return_once(|_| Ok(()));

        let result = apply_event(
            Arc::new(project_cluster),
            Arc::new(MockResourceDrivenCluster::new()),
            &evt.into(),
        )
        .await;

        assert!(matches!(result, Some(("project", Ok(())))));
    }
}
