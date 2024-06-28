mod bot;
mod bot_args;
mod metrics;
mod scraping_chains;

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

    let mut bot = Scraper::new(chain_infos, bot_args.fetch_cooldown, &registry);

    let metrics_rt = Runtime::new()?;
    metrics_rt.spawn(serve_metrics(registry.clone()));

    // Run long-running autocompound job.
    loop {
        // You can edit retries with CW_ORCH_MAX_TX_QUERY_RETRIES
        bot.scrape()?;

        // Wait for autocompound duration
        std::thread::sleep(bot.fetch_cooldown);

        // TODO: reconnect all daemons after sleep?
        // Maybe will be better to not build daemons until we actively use it

        // Reconnect
        // bot.daemon = Daemon::builder()
        //     .handle(rt.handle())
        //     .chain(chain_info.clone())
        //     .build()?;
    }
}
