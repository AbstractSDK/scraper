mod bot;
mod bot_args;
mod metrics;

pub use bot::Scraper;
pub use bot_args::BotArgs;
use cw_orch_interchain::{ChannelCreationValidator, DaemonInterchain, IbcQueryHandler};
use metrics::serve_metrics;
pub use metrics::Metrics;

use cw_orch::{
    anyhow,
    daemon::{networks::OSMOSIS_1, Daemon},
    tokio::runtime::Runtime,
};

use prometheus::Registry;

/// entrypoint for the bot
pub fn cron_main(bot_args: BotArgs) -> anyhow::Result<()> {
    let rt = Runtime::new()?;
    let registry = Registry::new();
    let chain_info = OSMOSIS_1;

    let daemon = Daemon::builder()
        .handle(rt.handle())
        .chain(chain_info.clone())
        .build()?;
    let daemons = vec![daemon];
    let chain_ids = daemons.iter().map(|daemon| daemon.chain_id()).collect();
    let interchain =
        DaemonInterchain::from_daemons(rt.handle(), daemons, &ChannelCreationValidator);
    let mut bot = Scraper::new(interchain, bot_args.fetch_cooldown, &registry, chain_ids);

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
