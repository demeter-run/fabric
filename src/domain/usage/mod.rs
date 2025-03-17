use std::sync::Arc;

use chrono::{DateTime, Datelike, Utc};
use tracing::{error, warn};
use uuid::Uuid;

use super::{event::UsageCreated, metadata::MetadataDriven};

pub mod cache;
pub mod cluster;
pub mod command;

pub struct Usage {
    pub id: String,
    pub event_id: String,
    pub resource_id: String,
    pub cluster_id: String,
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
                cluster_id: value.cluster_id.clone(),
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

pub trait UsageReportImpl {
    fn calculate_cost(&mut self, metadata: Arc<dyn MetadataDriven>) -> Self;
}
#[derive(Debug, Clone)]
pub struct UsageReport {
    pub project_id: String,
    pub project_namespace: String,
    pub project_billing_provider: String,
    pub project_billing_provider_id: String,
    pub resource_id: String,
    pub resource_kind: String,
    pub resource_name: String,
    pub resource_spec: String,
    pub tier: String,
    pub units: i64,
    pub interval: i64,
    pub period: String,
    pub units_cost: Option<f64>,
    pub minimum_cost: Option<f64>,
}
impl UsageReportImpl for Vec<UsageReport> {
    fn calculate_cost(&mut self, metadata: Arc<dyn MetadataDriven>) -> Self {
        let now = chrono::Utc::now();
        let next_month = if now.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap()
        } else {
            chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap()
        };
        let first_day = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
        let days = (next_month - first_day).num_days();
        let month_interval = (days * 24 * 60 * 60) as f64;

        self.iter_mut().for_each(|usage| {
            match metadata.find_by_kind(&usage.resource_kind) {
                Ok(metadata) => match metadata {
                    Some(metadata) => match metadata.cost.get(&usage.tier) {
                        Some(cost) => {
                            let value = (usage.units as f64) * cost.delta;
                            let rounded = (value * 100.0).round() / 100.0;

                            usage.units_cost = Some(rounded);

                            if cost.minimum > 0. {
                                let value =
                                    (cost.minimum / month_interval) * (usage.interval as f64);
                                let rounded = (value * 100.0).round() / 100.0;

                                usage.minimum_cost = Some(rounded);
                            }
                        }
                        None => {
                            warn!("tier cost not found for the kind {}", usage.resource_kind)
                        }
                    },
                    None => warn!("metadata not found for the kind {}", usage.resource_kind),
                },
                Err(error) => error!(?error, "fail to find the metadata"),
            };
        });

        self.to_vec()
    }
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
                cluster_id: Uuid::new_v4().to_string(),
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
                project_id: Uuid::new_v4().to_string(),
                project_namespace: "xxx".into(),
                project_billing_provider: "stripe".into(),
                project_billing_provider_id: "xxx".into(),
                resource_id: Uuid::new_v4().to_string(),
                resource_kind: "CardanoNodePort".into(),
                resource_name: format!("cardanonode-{}", utils::get_random_salt()),
                resource_spec:
                    "{\"version\":\"stable\",\"network\":\"mainnet\",\"throughputTier\":\"1\"}"
                        .into(),
                units: 120,
                interval: 60,
                tier: "0".into(),
                period: "2024-08".into(),
                units_cost: Some(0.),
                minimum_cost: Some(0.),
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
