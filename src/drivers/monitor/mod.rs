use anyhow::Result;
use kafka::{
    client::{FetchOffset, GroupOffsetStorage},
    consumer::Consumer,
};
use std::sync::Arc;
use tracing::info;

use crate::{
    domain::{daemon::namespace::create_namespace, events::Event},
    driven::k8s::K8sCluster,
};

pub async fn subscribe(kafka_host: &str) -> Result<()> {
    let k8s_cluster = Arc::new(K8sCluster::new().await?);

    let topic = "events".to_string();
    let hosts = &[kafka_host.into()];

    let mut consumer = Consumer::from_hosts(hosts.to_vec())
        .with_topic(topic.clone())
        .with_group("clusters".to_string())
        .with_fallback_offset(FetchOffset::Earliest)
        .with_offset_storage(Some(GroupOffsetStorage::Kafka))
        .create()?;

    info!("Subscriber running");

    loop {
        let mss = consumer.poll()?;
        if mss.is_empty() {
            continue;
        }

        for ms in mss.iter() {
            for m in ms.messages() {
                let event: Event = serde_json::from_slice(m.value)?;
                match event {
                    Event::NamespaceCreation(namespace) => {
                        create_namespace(k8s_cluster.clone(), namespace).await?;
                    }
                    Event::AccountCreation(_) => todo!(),
                };
            }
            consumer.consume_messageset(ms)?;
        }
        consumer.commit_consumed()?;
    }
}
