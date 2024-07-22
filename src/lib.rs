mod abstract_daemon_state;
mod args;
mod contract_state;
mod metrics;
mod scraper;

pub use args::ScraperArgs;
use metrics::serve_metrics;
pub use metrics::Metrics;
pub use scraper::{scraping_chains::ScrapingChains, Scraper};

use cw_orch::{
    anyhow,
    daemon::networks::{ARCHWAY_1, JUNO_1, PION_1},
    tokio::runtime::Runtime,
};

use prometheus::Registry;

/// entrypoint for the bot
pub fn cron_main(bot_args: ScraperArgs) -> anyhow::Result<()> {
    let registry = Registry::new();
    // TODO: We can't store daemons/interchain for long living task because of disconnect
    // Should be possible to replace ScrapingChains with DaemonInterchain with this:
    // https://github.com/AbstractSDK/cw-orchestrator/pull/352
    let chain_infos = ScrapingChains::new(vec![PION_1, JUNO_1, ARCHWAY_1]);

    let mut bot = Scraper::new(bot_args.fetch_cooldown, &registry);

    let metrics_rt = Runtime::new()?;
    metrics_rt.spawn(serve_metrics(registry.clone()));

    // Run long-running autocompound job.
    loop {
        // You can edit retries with CW_ORCH_MAX_TX_QUERY_RETRIES
        bot.scrape(&chain_infos)?;

        // Wait for autocompound duration
        std::thread::sleep(bot.fetch_cooldown);
    }
}
