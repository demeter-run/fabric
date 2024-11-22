use std::sync::Arc;

use anyhow::bail;
use axum::{http::StatusCode, response::IntoResponse};
use prometheus::TextEncoder;
use tracing::{error, info};

use crate::driven::prometheus::metrics::MetricsDriven;

pub async fn server(addr: &str, metrics: Arc<MetricsDriven>) -> anyhow::Result<()> {
    let app = axum::Router::new().route(
        "/metrics",
        axum::routing::get(|| async move {
            let encoder = TextEncoder::new();

            match encoder.encode_to_string(&metrics.metrics_collected()) {
                Ok(v) => v.into_response(),
                Err(error) => {
                    error!(?error);
                    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                }
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    info!(address = addr, "Metrics server running");
    if let Err(err) = axum::serve(listener, app).await {
        bail!(err);
    }

    Ok(())
}
