use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::event::UsageCreated;

pub mod cache;
pub mod cluster;

pub struct Usage {
    pub id: String,
    pub event_id: String,
    pub resource_id: String,
    pub units: i64,
    pub tier: String,
    pub created_at: DateTime<Utc>,
}
impl From<UsageCreated> for Vec<Usage> {
    fn from(evt: UsageCreated) -> Self {
        evt.usages
            .iter()
            .map(|usage| Usage {
                id: Uuid::new_v4().to_string(),
                event_id: evt.id.clone(),
                resource_id: usage.resource_id.clone(),
                units: usage.units,
                tier: usage.tier.clone(),
                created_at: evt.created_at,
            })
            .collect()
    }
}

pub struct UsageUnit {
    pub resource_id: String,
    pub units: i64,
    pub tier: String,
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
                created_at: Utc::now(),
            }
        }
    }
}
