use chrono::{DateTime, Utc};

use super::event::ResourceCreated;

pub mod cache;
pub mod cluster;
pub mod command;

pub struct Resource {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl From<ResourceCreated> for Resource {
    fn from(value: ResourceCreated) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            kind: value.kind,
            data: value.data,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    impl Default for Resource {
        fn default() -> Self {
            Self {
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                kind: "CardanoNode".into(),
                data: "{\"spec\":{\"operatorVersion\":\"1\",\"kupoVersion\":\"v1\",\"network\":\"mainnet\",\"pruneUtxo\":false,\"throughputTier\":\"0\"}}".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        }
    }
}
