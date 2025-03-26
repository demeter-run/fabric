use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::error;

use crate::{
    domain::{
        error::Error,
        usage::{cluster::UsageDrivenCluster, UsageResourceUnit},
        Result,
    },
    driven::prometheus::deserialize_value,
};

use super::PrometheusUsageDriven;

#[async_trait::async_trait]
impl UsageDrivenCluster for PrometheusUsageDriven {
    async fn find_metrics(
        &self,
        project_name: &str,
        resource_name: &str,
        step: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageResourceUnit>> {
        let response = self
            .client
            .get(format!(
                "{}/query_range?query=round(usage{{project=\"{project_name}\",resource_name=\"{resource_name}\"}})",
                &self.url
            ))
            .query(&[
                ("start", start.timestamp().to_string()),
                ("end", end.timestamp().to_string()),
                ("step", step.into()),
            ])
            .send()
            .await?;

        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            error!(status = status.to_string(), "request status code fail");
            return Err(Error::Unexpected(format!(
                "Prometheus request error. Status: {}",
                status
            )));
        }

        let response: PrometheusResponse = response.json().await?;

        let units = response.data.result.iter().map(|r| {
            let min = r.values.iter().min_by_key(|v| v.timestamp);
            let max = r.values.iter().max_by_key(|v| v.timestamp);

            let first_timestamp = match min {
                Some(v) => v.timestamp,
                None => 0,
            };
            let last_timestamp = match max {
                Some(v) => v.timestamp,
                None => 0,
            };

            let first_value = match min {
                Some(v) => v.value,
                None => 0,
            };
            let last_value = match max {
                Some(v) => v.value,
                None => 0,
            };

            let interval = last_timestamp - first_timestamp;
            let units = match last_value - first_value {
                v if v < 0 => 0,
                v => v,
            };

            UsageResourceUnit {
                units,
                interval,
                tier: r.metric.tier.clone(),
            }
        });

        Ok(units.collect())
    }
}

#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    data: PrometheusData,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrometheusData {
    result: Vec<PrometheusUsageResult>,
}
#[derive(Debug, Deserialize)]
struct PrometheusUsageResult {
    metric: PrometheusUsageMetric,
    values: Vec<PrometheusValue>,
}
#[derive(Debug, Deserialize)]
struct PrometheusValue {
    #[serde(rename = "0")]
    timestamp: u64,

    #[serde(rename = "1")]
    #[serde(deserialize_with = "deserialize_value")]
    value: i64,
}
#[derive(Debug, Deserialize)]
pub struct PrometheusUsageMetric {
    tier: String,
}
