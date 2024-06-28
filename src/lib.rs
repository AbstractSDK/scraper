mod bot;
mod bot_args;
mod metrics;

pub use bot::Scraper;
pub use bot_args::BotArgs;
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
    let mut chain_info = OSMOSIS_1;
    let grpc_urls = if !bot_args.grps_urls.is_empty() {
        bot_args.grps_urls.iter().map(String::as_ref).collect()
    } else {
        chain_info.grpc_urls.to_vec()
    };

    chain_info.grpc_urls = &grpc_urls;
    let daemon = Daemon::builder()
        .handle(rt.handle())
        .chain(chain_info.clone())
        .build()?;

    let module_info =
        ModuleInfo::from_id(APP_ID, ModuleVersion::Version(APP_VERSION.parse().unwrap()))?;

    let mut bot = Scraper::new(
        daemon,
        module_info,
        bot_args.fetch_cooldown,
        bot_args.autocompound_cooldown,
        &registry,
    );

    let metrics_rt = Runtime::new()?;
    metrics_rt.spawn(serve_metrics(registry.clone()));

    // Run long-running autocompound job.
    loop {
        // You can edit retries with CW_ORCH_MAX_TX_QUERY_RETRIES
        bot.scrape()?;

        // Wait for autocompound duration
        std::thread::sleep(bot.autocompound_cooldown);

        // Reconnect
        bot.daemon = Daemon::builder()
            .handle(rt.handle())
            .chain(chain_info.clone())
            .build()?;
    }
}
