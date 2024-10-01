use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::event::UsageUnitCreated;

pub mod cache;
pub mod cluster;
pub mod command;

pub struct Usage {
    pub id: String,
    pub event_id: String,
    pub resource_id: String,
    pub units: i64,
    pub tier: String,
    pub interval: u64,
    pub created_at: DateTime<Utc>,
}
impl Usage {
    pub fn from_usage_evt(
        usage: &UsageUnitCreated,
        resource_id: &str,
        evt_id: &str,
        evt_created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_id: evt_id.into(),
            resource_id: resource_id.into(),
            units: usage.units,
            tier: usage.tier.clone(),
            interval: usage.interval,
            created_at: evt_created_at,
        }
    }
}

#[derive(Debug)]
pub struct UsageMetric {
    pub project_namespace: String,
    pub resources: Vec<UsageUnitMetric>,
}
#[derive(Debug, Clone)]
pub struct UsageUnitMetric {
    pub resource_name: String,
    pub units: i64,
    pub tier: String,
    pub interval: u64,
}

#[derive(Debug)]
pub struct UsageReport {
    pub resource_id: String,
    pub resource_kind: String,
    pub resource_spec: String,
    pub tier: String,
    pub units: i64,
    pub period: String,
}

#[derive(Debug)]
pub struct UsageReportAggregated {
    pub project_id: String,
    pub project_namespace: String,
    #[allow(dead_code)]
    pub project_billing_provider: String,
    pub project_billing_provider_id: String,
    pub resource_id: String,
    pub resource_kind: String,
    pub tier: String,
    pub interval: u64,
    pub units: i64,
    #[allow(dead_code)]
    pub period: String,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    impl Default for Usage {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                event_id: Uuid::new_v4().to_string(),
                resource_id: Uuid::new_v4().to_string(),
                units: 120,
                tier: "0".into(),
                interval: 10,
                created_at: Utc::now(),
            }
        }
    }

    impl Default for UsageReport {
        fn default() -> Self {
            Self {
                resource_id: Uuid::new_v4().to_string(),
                resource_kind: "CardanoNodePort".into(),
                resource_spec:
                    "{\"version\":\"stable\",\"network\":\"mainnet\",\"throughputTier\":\"1\"}"
                        .into(),
                units: 120,
                tier: "0".into(),
                period: "08-2024".into(),
            }
        }
    }
}
