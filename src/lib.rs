mod abstract_state;
mod bot;
mod bot_args;
mod contract_state;
mod metrics;
mod scraping_chains;

use abstract_state::AbstractState;
pub use bot::Scraper;
pub use bot_args::BotArgs;
use cw_orch_interchain::{ChannelCreationValidator, IbcQueryHandler};
use metrics::serve_metrics;
pub use metrics::Metrics;
pub use scraping_chains::ScrapingChains;

use cw_orch::{
    anyhow,
    daemon::{
        networks::{JUNO_1, OSMOSIS_1, PION_1},
        Daemon,
    },
    tokio::runtime::Runtime,
};

use prometheus::Registry;

/// entrypoint for the bot
pub fn cron_main(bot_args: BotArgs) -> anyhow::Result<()> {
    let registry = Registry::new();
    // TODO: We can't store daemons/interchain for long living task because of disconnect
    // Should be possible to replace ScrapingChains with DaemonInterchain with this:
    // https://github.com/AbstractSDK/cw-orchestrator/pull/352
    let chain_infos = ScrapingChains::new(vec![PION_1]);
    let abstract_state = AbstractState::default();

    let mut bot = Scraper::new(
        chain_infos,
        bot_args.fetch_cooldown,
        &registry,
        abstract_state,
    );

    let metrics_rt = Runtime::new()?;
    metrics_rt.spawn(serve_metrics(registry.clone()));

    // Run long-running autocompound job.
    loop {
        // You can edit retries with CW_ORCH_MAX_TX_QUERY_RETRIES
        bot.scrape()?;

        // Wait for autocompound duration
        std::thread::sleep(bot.fetch_cooldown);
    }
}
