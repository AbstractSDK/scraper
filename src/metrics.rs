use prometheus::{core::AtomicU64, Encoder, IntCounter, Opts, Registry, TextEncoder};

use warp::Filter;

pub type UIntGaugeVec = prometheus::core::GenericGaugeVec<AtomicU64>;

pub struct Metrics {
    pub fetch_count: IntCounter,
    pub local_account_instances_count: UIntGaugeVec,
    pub remote_account_instances_count: UIntGaugeVec,
    pub contracts_by_namespace_count: UIntGaugeVec,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        let fetch_count = IntCounter::new(
            "scraper_app_bot_fetch_count",
            "Number of times the bot has fetched the instances",
        )
        .unwrap();
        let local_account_instances_count = UIntGaugeVec::new(
            Opts::new(
                "scraper_bot_local_account_instances_count",
                "Number of local account instances",
            ),
            &["chain_id"],
        )
        .unwrap();
        let remote_account_instances_count = UIntGaugeVec::new(
            Opts::new(
                "scraper_bot_remote_account_instances_count",
                "Number of remote account instances",
            ),
            &["chain_id"],
        )
        .unwrap();
        let contracts_by_namespace_count = UIntGaugeVec::new(
            Opts::new(
                "scraper_bot_contracts_by_namespace",
                "Number of contracts by namespace",
            ),
            &["chain_id", "namespace"],
        )
        .unwrap();

        registry.register(Box::new(fetch_count.clone())).unwrap();
        registry
            .register(Box::new(local_account_instances_count.clone()))
            .unwrap();
        registry
            .register(Box::new(remote_account_instances_count.clone()))
            .unwrap();
        registry
            .register(Box::new(contracts_by_namespace_count.clone()))
            .unwrap();
        Self {
            fetch_count,
            local_account_instances_count,
            remote_account_instances_count,
            contracts_by_namespace_count,
        }
    }
}

pub async fn serve_metrics(registry: prometheus::Registry) {
    let addr: std::net::SocketAddr = "0.0.0.0:80".parse().unwrap();
    let metric_server = warp::serve(warp::path("metrics").map(move || {
        let metric_families = registry.gather();
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        warp::reply::with_header(
            buffer,
            "content-type",
            "text/plain; version=0.0.4; charset=utf-8",
        )
    }));
    metric_server.run(addr).await;
}
