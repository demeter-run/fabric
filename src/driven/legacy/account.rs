use anyhow::Result;
use std::sync::Arc;

use crate::domain::management::account::{Account, DrivenLegacyAccount};

use super::DemeterLegacy;

pub struct LegacyAccount {
    demeter_legacy: Arc<DemeterLegacy>,
}
impl LegacyAccount {
    pub fn new(demeter_legacy: Arc<DemeterLegacy>) -> Self {
        Self { demeter_legacy }
    }
}
impl DrivenLegacyAccount for LegacyAccount {
    async fn create(&self, account: &Account) -> Result<()> {
        Ok(())
    }
}
