use clap::Parser;
use humantime::parse_duration;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct ScraperArgs {
    /// Fetch cooldown
    #[arg(long = "fcd", value_parser = parse_duration, value_name = "DURATION")]
    pub fetch_cooldown: Duration,
}
