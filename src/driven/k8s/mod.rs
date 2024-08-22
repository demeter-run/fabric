use anyhow::Result as AnyhowResult;
use k8s_openapi::api::core::v1::Namespace;
use kube::{
    api::{DeleteParams, DynamicObject, Patch, PatchParams, PostParams},
    discovery, Api, Client, Error, ResourceExt,
};
use tracing::{info, warn};

use crate::domain::{
    project::cluster::ProjectDrivenCluster, resource::cluster::ResourceDrivenCluster, Result,
};

pub struct K8sCluster {
    client: Client,
}
impl K8sCluster {
    pub async fn new() -> AnyhowResult<Self> {
        let client = Client::try_default().await?;

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl ProjectDrivenCluster for K8sCluster {
    async fn create(&self, namespace: &Namespace) -> Result<()> {
        let api: Api<Namespace> = Api::all(self.client.clone());
        if let Err(err) = api.create(&PostParams::default(), namespace).await {
            match &err {
                Error::Api(error_response) => {
                    if error_response.reason == "AlreadyExists" {
                        info!("Resource already exists, skipping.")
                    } else {
                        return Err(err.into());
                    }
                }
                _ => return Err(err.into()),
            }
        };

        Ok(())
    }

    async fn delete(&self, namespace: &Namespace) -> Result<()> {
        let api: Api<Namespace> = Api::all(self.client.clone());
        if let Err(err) = api
            .delete(&namespace.name_any(), &DeleteParams::default())
            .await
        {
            match &err {
                Error::Api(error_response) => {
                    if error_response.reason == "NotFound" {
                        info!("Resource not found in cluster, skipping.")
                    } else {
                        return Err(err.into());
                    }
                }
                _ => return Err(err.into()),
            }
        };

        Ok(())
    }
}

#[async_trait::async_trait]
impl ResourceDrivenCluster for K8sCluster {
    async fn create(&self, obj: &DynamicObject) -> Result<()> {
        let apigroup = discovery::group(&self.client, "demeter.run").await?;
        let kind = &obj.types.as_ref().unwrap().kind;
        let (ar, _caps) = match apigroup.recommended_kind(kind) {
            Some((ar, _caps)) => (ar, _caps),
            None => {
                warn!(kind = kind, "Coundnt find kind in cluster, skipping.");
                return Ok(());
            }
        };

        let api: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), &obj.namespace().unwrap(), &ar);

        if let Err(err) = api.create(&PostParams::default(), obj).await {
            match &err {
                Error::Api(error_response) => {
                    if error_response.reason == "AlreadyExists" {
                        info!("Resource already exists, skipping.")
                    } else {
                        return Err(err.into());
                    }
                }
                _ => return Err(err.into()),
            }
        };
        Ok(())
    }

    async fn update(&self, obj: &DynamicObject) -> Result<()> {
        let apigroup = discovery::group(&self.client, "demeter.run").await?;
        let kind = &obj.types.as_ref().unwrap().kind;
        let (ar, _caps) = match apigroup.recommended_kind(kind) {
            Some((ar, _caps)) => (ar, _caps),
            None => {
                warn!(kind = kind, "Coundnt find kind in cluster, skipping.");
                return Ok(());
            }
        };

        let api: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), &obj.namespace().unwrap(), &ar);

        api.patch(
            &obj.name_any(),
            &PatchParams::default(),
            &Patch::Merge(obj.data.clone()),
        )
        .await?;

        Ok(())
    }

    async fn delete(&self, obj: &DynamicObject) -> Result<()> {
        let apigroup = discovery::group(&self.client, "demeter.run").await?;
        let kind = &obj.types.as_ref().unwrap().kind;
        let (ar, _caps) = match apigroup.recommended_kind(kind) {
            Some((ar, _caps)) => (ar, _caps),
            None => {
                warn!(kind = kind, "Coundnt find kind in cluster, skipping.");
                return Ok(());
            }
        };

        let api: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), &obj.namespace().unwrap(), &ar);

        if let Err(err) = api.delete(&obj.name_any(), &DeleteParams::default()).await {
            match &err {
                Error::Api(error_response) => {
                    if error_response.reason == "NotFound" {
                        info!("Resource not found in cluster, skipping.")
                    } else {
                        return Err(err.into());
                    }
                }
                _ => return Err(err.into()),
            }
        };

        Ok(())
    }
}
