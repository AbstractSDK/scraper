mod scrape_data;
pub mod scraping_chains;
mod utils;

use crate::{abstract_daemon_state::AbstractDaemonState, Metrics};

use cw_orch::{anyhow, prelude::*};
use log::{log, Level};
use prometheus::{labels, Registry};
use scrape_data::ScrapedData;
use scraping_chains::ScrapingChains;
use std::time::{Duration, SystemTime};

pub struct Scraper {
    // Fetch information
    pub fetch_cooldown: Duration,
    last_fetch: SystemTime,
    // metrics
    metrics: Metrics,
    abstract_state: AbstractDaemonState,
}

impl Scraper {
    pub fn new(fetch_cooldown: Duration, registry: &Registry) -> Self {
        let metrics = Metrics::new(registry);

        Self {
            fetch_cooldown,
            last_fetch: SystemTime::UNIX_EPOCH,
            metrics,
            abstract_state: AbstractDaemonState::default(),
        }
    }

    // Fetches contracts and assets if fetch cooldown passed
    pub fn scrape(&mut self, scraping_chains: &ScrapingChains) -> anyhow::Result<()> {
        // Don't fetch if not ready
        let ready_time = self.last_fetch + self.fetch_cooldown;
        if SystemTime::now() < ready_time {
            return Ok(());
        }

        log!(Level::Info, "Fetching contracts and assets");

        for daemon_result in scraping_chains.iter() {
            match daemon_result {
                Ok(daemon) => {
                    let env_info = daemon.env_info();
                    let scraped_data = ScrapedData::scrape_data(&daemon, &self.abstract_state);
                    self.update_metrics(&env_info.chain_id, scraped_data);
                }
                Err(e) => {
                    log::error!("{e}");
                }
            }
        }

        Ok(())
    }

    fn update_metrics(&mut self, chain_id: &str, scraped_data: ScrapedData) {
        self.metrics.fetch_count.inc();
        let label = labels! {"chain_id" => chain_id};

        // Update count of local accounts
        self.metrics
            .local_account_instances_count
            .with(&label)
            .set(scraped_data.account_local_instances.len() as u64);

        // Update count of remote accounts
        self.metrics
            .remote_account_instances_count
            .with(&label)
            .set(scraped_data.account_remote_instances.len() as u64);

        // Contracts by namespace count
        for (namespace, modules) in scraped_data.modules_by_namespace {
            let label = labels! {"chain_id" => chain_id, "namespace" => namespace.as_str()};
            self.metrics
                .contracts_by_namespace_count
                .with(&label)
                .set(modules.len() as u64)
        }
    }
}
