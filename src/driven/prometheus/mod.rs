use reqwest::Client;
use serde::{Deserialize, Deserializer};
use tracing::error;

use crate::domain::Result;

pub mod metrics;
pub mod usage;

pub struct PrometheusUsageDriven {
    client: Client,
    url: String,
}
impl PrometheusUsageDriven {
    pub fn new(url: &str) -> Self {
        let client = Client::new();
        let url = url.to_string();

        Self { client, url }
    }
}

fn deserialize_value<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.parse::<i64>() {
        Ok(v) => v,
        Err(error) => {
            error!(
                error = error.to_string(),
                value, "fail to convert prometheus value"
            );
            0
        }
    })
}
