use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::event::UsageCreated;

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
impl From<UsageCreated> for Vec<Usage> {
    fn from(value: UsageCreated) -> Self {
        value
            .usages
            .into_iter()
            .map(|u| Usage {
                id: Uuid::new_v4().to_string(),
                event_id: value.id.clone(),
                resource_id: u.resource_id,
                units: u.units,
                tier: u.tier.clone(),
                interval: u.interval,
                created_at: value.created_at,
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct UsageMetric {
    pub project_id: String,
    pub project_namespace: String,
    pub resources: Vec<UsageUnitMetric>,
}
#[derive(Debug, Clone)]
pub struct UsageUnitMetric {
    pub resource_id: String,
    pub resource_name: String,
    pub units: i64,
    pub tier: String,
    pub interval: u64,
}

#[derive(Debug)]
pub struct UsageReport {
    pub resource_id: String,
    pub resource_kind: String,
    pub resource_name: String,
    pub resource_spec: String,
    pub tier: String,
    pub units: i64,
    pub period: String,
}

#[derive(Debug)]
pub struct UsageResource {
    pub project_id: String,
    pub project_namespace: String,
    pub resource_id: String,
    pub resource_name: String,
    pub resource_spec: String,
}

#[derive(Debug, Clone)]
pub struct UsageResourceUnit {
    pub units: i64,
    pub tier: String,
    pub interval: u64,
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
    pub resource_name: String,
    pub tier: String,
    pub interval: u64,
    pub units: i64,
    #[allow(dead_code)]
    pub period: String,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::domain::utils;

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
                resource_name: format!("cardanonode-{}", utils::get_random_salt()),
                resource_spec:
                    "{\"version\":\"stable\",\"network\":\"mainnet\",\"throughputTier\":\"1\"}"
                        .into(),
                units: 120,
                tier: "0".into(),
                period: "08-2024".into(),
            }
        }
    }

    impl Default for UsageResource {
        fn default() -> Self {
            Self {
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "xxx".into(),
                resource_id: Uuid::new_v4().to_string(),
                resource_name: format!("cardanonode-{}", utils::get_random_salt()),
                resource_spec:
                    "{\"version\":\"stable\",\"network\":\"mainnet\",\"throughputTier\":\"1\"}"
                        .into(),
            }
        }
    }
}
