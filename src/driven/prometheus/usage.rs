use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::error;

use crate::{
    domain::{
        error::Error,
        usage::{cluster::UsageDrivenCluster, UsageUnit},
        Result,
    },
    driven::prometheus::deserialize_value,
};

use super::PrometheusUsageDriven;

#[async_trait::async_trait]
impl UsageDrivenCluster for PrometheusUsageDriven {
    async fn find_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageUnit>> {
        let since = (end - start).num_seconds();

        let query = format!(
            "round(sum by (resource_name, tier) (increase(usage{{tier!~\"0\"}}[{since}s] @ {})) > 0)",
            end.timestamp_millis() / 1000
        );

        let response = self
            .client
            .get(format!("{}/query?query={query}", &self.url))
            .send()
            .await?;

        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            error!(status = status.to_string(), "request status code fail");
            return Err(Error::Unexpected(format!(
                "Prometheus request error. Status: {} Query: {}",
                status, query
            )));
        }

        let response: PrometheusResponse = response.json().await?;

        let usage_units: Vec<UsageUnit> = response
            .data
            .result
            .iter()
            .map(|r| UsageUnit {
                resource_id: r.metric.resource_name.clone(),
                units: r.value,
                tier: r.metric.tier.clone(),
            })
            .collect();

        Ok(usage_units)
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
    #[serde(deserialize_with = "deserialize_value")]
    value: i64,
}
#[derive(Debug, Deserialize)]
pub struct PrometheusUsageMetric {
    resource_name: String,
    tier: String,
}
