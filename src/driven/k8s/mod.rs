use anyhow::Result;
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::PostParams, Api, Client};

use crate::domain::daemon::namespace::NamespaceCluster;

pub struct K8sCluster {
    client: Client,
}
impl K8sCluster {
    pub async fn new() -> Result<Self> {
        let client = Client::try_default().await?;

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl NamespaceCluster for K8sCluster {
    async fn create(&self, namespace: &Namespace) -> Result<()> {
        let api: Api<Namespace> = Api::all(self.client.clone());
        api.create(&PostParams::default(), namespace).await?;
        Ok(())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Namespace>> {
        let api: Api<Namespace> = Api::all(self.client.clone());

        Ok(api.get_opt(name).await?)
    }
}
