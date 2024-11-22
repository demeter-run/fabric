use anyhow::Result as AnyhowResult;
use prometheus::{opts, IntCounterVec, Registry};

pub struct MetricsDriven {
    registry: Registry,
    pub domain_errors: IntCounterVec,
}

impl MetricsDriven {
    pub fn new() -> AnyhowResult<Self> {
        let registry = Registry::default();

        let domain_errors = IntCounterVec::new(
            opts!("fabric_domain_errors_total", "fabric domain errors",),
            &["source", "domain", "error"],
        )
        .unwrap();
        registry.register(Box::new(domain_errors.clone()))?;

        Ok(Self {
            registry,
            domain_errors,
        })
    }

    pub fn metrics_collected(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    pub fn domain_error(&self, source: &str, domain: &str, error: &str) {
        self.domain_errors
            .with_label_values(&[source, domain, error])
            .inc()
    }
}
