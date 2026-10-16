use dmtri::demeter::ops::v1alpha as proto;
use std::sync::Arc;
use tonic::async_trait;

use crate::{
    domain::{
        event::EventDrivenBridge,
        project::cache::ProjectDrivenCache,
        provisional::{
            command::{self, RegisterCmd},
            ProvisionalConfig,
        },
    },
    driven::prometheus::metrics::MetricsDriven,
};

use super::handle_error_metric;

/// Anonymous registration, for the Console's agent routes only. Mounted without the user
/// interceptor: a caller is authorized by the gateway token, which `register` checks.
pub struct ProvisionalServiceImpl {
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    metrics: Arc<MetricsDriven>,
    config: ProvisionalConfig,
    secret: String,
}
impl ProvisionalServiceImpl {
    pub fn new(
        cache: Arc<dyn ProjectDrivenCache>,
        event: Arc<dyn EventDrivenBridge>,
        metrics: Arc<MetricsDriven>,
        config: ProvisionalConfig,
        secret: String,
    ) -> Self {
        Self {
            cache,
            event,
            metrics,
            config,
            secret,
        }
    }
}

fn metadata_string<T>(request: &tonic::Request<T>, key: &str) -> Option<String> {
    let value = request.metadata().get(key)?;
    value.to_str().ok().map(String::from)
}

#[async_trait]
impl proto::provisional_service_server::ProvisionalService for ProvisionalServiceImpl {
    async fn register(
        &self,
        request: tonic::Request<proto::RegisterRequest>,
    ) -> Result<tonic::Response<proto::RegisterResponse>, tonic::Status> {
        let gateway_token = metadata_string(&request, "dmtr-gateway-token");
        let client_ip_metadata = metadata_string(&request, "dmtr-client-ip");

        let req = request.into_inner();

        // The request field is the client IP; the contract also names a metadata key for it.
        let client_ip = if req.client_ip.is_empty() {
            client_ip_metadata.unwrap_or_default()
        } else {
            req.client_ip
        };

        let cmd = RegisterCmd {
            gateway_token,
            agent_name: req.agent_name,
            agent_version: req.agent_version,
            agent_url: req.agent_url,
            client_ip,
        };

        let registration = command::register(
            self.cache.clone(),
            self.event.clone(),
            &self.config,
            &self.secret,
            cmd,
        )
        .await
        .inspect_err(|err| handle_error_metric(self.metrics.clone(), "provisional", err))?;

        let message = proto::RegisterResponse {
            project_id: registration.project_id,
            project_namespace: registration.project_namespace,
            api_key: registration.api_key,
            claim_token: registration.claim_token,
            user_code: registration.user_code,
            expires_at: registration.expires_at.to_rfc3339(),
        };

        Ok(tonic::Response::new(message))
    }
}
