use anyhow::Result;
use kafka::{client::{FetchOffset, GroupOffsetStorage}, consumer::Consumer};
use tracing::info;

pub async fn subscribe() -> Result<()> {
    let topic = "events".to_string();
    let hosts = &["localhost:19092".into()];

    let mut consumer = Consumer::from_hosts(hosts.to_vec())
        .with_topic(topic.clone())
        .with_group("c1".to_string())
        .with_fallback_offset(FetchOffset::Earliest)
        .with_offset_storage(Some(GroupOffsetStorage::Kafka))
        .create()?;

    info!("Event Driver started listening");

    loop {
        let result = consumer.poll();
        if let Err(err) = result {
            dbg!(&err);
            return Err(err.into());
        }
        //let mss = consumer.poll()?;
        //dbg!(&mss);
        //if mss.is_empty() {
        //    println!("No messages available right now.");
        //    return Ok(());
        //}
        //
        //for ms in mss.iter() {
        //    for m in ms.messages() {
        //        dbg!(m);
        //    }
        //    let _ = consumer.consume_messageset(ms);
        //}
        //consumer.commit_consumed()?;
    }
}
