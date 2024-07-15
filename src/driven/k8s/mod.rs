use anyhow::Result;
use k8s_openapi::api::core::v1::Namespace;
use kube::ResourceExt;
use kube::{
    api::{DynamicObject, PostParams},
    discovery, Api, Client,
};

use crate::domain::ports::PortCluster;
use crate::domain::projects::ProjectCluster;

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
impl ProjectCluster for K8sCluster {
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

#[async_trait::async_trait]
impl PortCluster for K8sCluster {
    async fn create(&self, port: &DynamicObject) -> Result<()> {
        let apigroup = discovery::group(&self.client, "demeter.run").await?;
        let (ar, _caps) = apigroup
            .recommended_kind(&port.types.as_ref().unwrap().kind)
            .unwrap();

        let api: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), &port.namespace().unwrap(), &ar);

        api.create(&PostParams::default(), port).await?;
        Ok(())
    }
}
