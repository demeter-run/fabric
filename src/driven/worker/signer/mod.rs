use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use tracing::instrument;
use vaultrs::{
    api::transit::requests::{ExportKeyType, ExportVersion},
    client::{VaultClient, VaultClientSettingsBuilder},
    token,
    transit::{data, key},
};

use crate::domain::{
    error::Error,
    worker::signer::{Signer, WorkerSignerDrivenStorage},
    Result,
};

pub struct VaultWorkerSignerDrivenStorage {
    client: VaultClient,
}
impl VaultWorkerSignerDrivenStorage {
    pub fn try_new(address: &str, token: &str) -> Result<Self, Error> {
        let settings = VaultClientSettingsBuilder::default()
            .address(address)
            .token(token)
            .verify(false)
            .build()?;
        let client = VaultClient::new(settings.clone())?;

        // Background thread for token renewal.
        tokio::spawn(run(VaultClient::new(settings.clone())?));
        Ok(Self { client })
    }

    pub fn key_for_worker(worker_id: &str, key_name: &str) -> String {
        format!("{worker_id}-{key_name}")
    }
}

#[async_trait::async_trait]
impl WorkerSignerDrivenStorage for VaultWorkerSignerDrivenStorage {
    async fn list(&self, worker_id: &str) -> Result<Vec<Signer>> {
        let response = key::list(&self.client, "transit").await?;
        Ok(response
            .keys
            .iter()
            .filter(|key| key.starts_with(worker_id))
            .map(|key| Signer {
                worker_id: worker_id.to_string(),
                key_name: key.to_owned(),
            })
            .collect())
    }

    async fn get_public_key(&self, worker_id: &str, key_name: String) -> Result<Option<Vec<u8>>> {
        let vault_key = Self::key_for_worker(worker_id, &key_name);
        let response = key::export(
            &self.client,
            "transit",
            &vault_key,
            ExportKeyType::PublicKey,
            ExportVersion::Latest,
        )
        .await?;
        response
            .keys
            .get("1")
            .map(|key| {
                STANDARD.decode(key).map_err(|err| {
                    Error::Unexpected(format!("Failed to decode vault response: {:?}", err))
                })
            })
            .transpose()
    }

    async fn sign_payload(
        &self,
        worker_id: &str,
        key_name: String,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let vault_key = Self::key_for_worker(worker_id, &key_name);
        let response = data::sign(
            &self.client,
            "transit",
            &vault_key,
            &STANDARD.encode(payload),
            None,
        )
        .await?;
        STANDARD
            .decode(response.signature.replace("vault:v1:", ""))
            .map_err(|err| Error::Unexpected(err.to_string()))
    }
}

#[instrument("vault-token-renewer", skip_all)]
pub async fn run(client: VaultClient) -> Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        match token::renew_self(&client, Some("1h")).await {
            Ok(response) => {
                tracing::debug!(response =? response, "renewed vault token");
            }
            Err(err) => {
                tracing::error!(err =? err, "failed to renew vault token");
            }
        };
    }
}
