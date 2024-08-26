use anyhow::Result as AnyhowResult;
use reqwest::Client;
use serde::{Deserialize, Deserializer};

use crate::domain::Result;

pub mod usage;

pub struct PrometheusUsageDriven {
    client: Client,
    url: String,
}
impl PrometheusUsageDriven {
    pub async fn new(url: &str) -> AnyhowResult<Self> {
        let client = Client::new();
        let url = url.to_string();

        Ok(Self { client, url })
    }
}

fn deserialize_value<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Vec<serde_json::Value> = Deserialize::deserialize(deserializer)?;
    Ok(value.into_iter().as_slice()[1]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap())
}
