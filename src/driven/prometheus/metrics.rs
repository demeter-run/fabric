use anyhow::Result as AnyhowResult;
use prometheus::{opts, IntCounter, IntCounterVec, IntGauge, Registry};

pub struct MetricsDriven {
    registry: Registry,
    pub domain_errors: IntCounterVec,
    pub usage_collected: IntCounter,
    pub provisional_sweep_runs: IntCounterVec,
    pub provisional_projects_expired: IntCounter,
    pub provisional_projects_overdue: IntGauge,
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

        let provisional_sweep_runs = IntCounterVec::new(
            opts!(
                "fabric_provisional_sweep_runs_total",
                "provisional expiry sweep ticks by outcome",
            ),
            &["outcome"],
        )
        .unwrap();
        registry.register(Box::new(provisional_sweep_runs.clone()))?;

        let provisional_projects_expired = IntCounter::new(
            "fabric_provisional_projects_expired_total",
            "provisional projects expired by the sweep",
        )
        .unwrap();
        registry.register(Box::new(provisional_projects_expired.clone()))?;

        let provisional_projects_overdue = IntGauge::new(
            "fabric_provisional_projects_overdue",
            "provisional projects still unexpired more than one sweep interval past their expiry",
        )
        .unwrap();
        registry.register(Box::new(provisional_projects_overdue.clone()))?;

        Ok(Self {
            registry,
            domain_errors,
            usage_collected,
            provisional_sweep_runs,
            provisional_projects_expired,
            provisional_projects_overdue,
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

    pub fn provisional_sweep_ok(&self, expired: usize, overdue: usize) {
        self.provisional_sweep_runs.with_label_values(&["ok"]).inc();
        self.provisional_projects_expired.inc_by(expired as u64);
        self.provisional_projects_overdue.set(overdue as i64);
    }

    pub fn provisional_sweep_error(&self) {
        self.provisional_sweep_runs
            .with_label_values(&["error"])
            .inc();
    }
}
