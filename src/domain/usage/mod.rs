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
    pub created_at: DateTime<Utc>,
}
impl From<UsageCreated> for Vec<Usage> {
    fn from(evt: UsageCreated) -> Self {
        evt.resources
            .iter()
            .map(|(resource_id, units)| Usage {
                id: Uuid::new_v4().to_string(),
                event_id: evt.id.clone(),
                resource_id: resource_id.into(),
                units: *units,
                created_at: evt.created_at,
            })
            .collect()
    }
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
                created_at: Utc::now(),
            }
        }
    }
}
