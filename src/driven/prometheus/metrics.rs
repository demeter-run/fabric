use anyhow::Result as AnyhowResult;
use prometheus::{opts, IntCounter, IntCounterVec, Registry};

pub struct MetricsDriven {
    registry: Registry,
    pub domain_errors: IntCounterVec,
    pub usage_collected: IntCounter,
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

        let usage_collected = IntCounter::new(
            "fabric_domain_usage_total_units",
            "fabric domain usage total units collected",
        )
        .unwrap();
        registry.register(Box::new(usage_collected.clone()))?;

        Ok(Self {
            registry,
            domain_errors,
            usage_collected,
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

    pub fn domain_usage_collected(&self, total_units: u64) {
        self.usage_collected.inc_by(total_units)
    }
}
