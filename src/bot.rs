use abstract_client::{AbstractClient, AccountSource, Environment};
use cw_asset::AssetInfo;
use cw_orch_interchain::DaemonInterchain;
use semver::VersionReq;

use crate::Metrics;
use cosmos_sdk_proto::cosmwasm::wasm::v1::{
    query_client::QueryClient, QueryContractsByCodeRequest,
};

use cosmwasm_std::Uint128;
use cw_orch::{
    anyhow,
    daemon::{queriers::Authz, Daemon},
    prelude::*,
};
use log::{log, Level};
use prometheus::{labels, Registry};
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter},
    time::{Duration, SystemTime},
};
use tonic::transport::Channel;

pub struct Scraper {
    // Fetch information
    pub fetch_cooldown: Duration,
    last_fetch: SystemTime,
    // Chains to Fetch
    interchain: DaemonInterchain,
    // metrics
    metrics: Metrics,
    // proxy instances
    // Chain id -> proxy addresses
    proxy_instances: HashMap<String, Vec<String>>,
}

#[derive(Eq, Hash, PartialEq, Clone)]
struct CarrotInstance {
    address: Addr,
    version: String,
}
impl CarrotInstance {
    fn new(address: Addr, version: &str) -> Self {
        Self {
            address,
            version: version.to_string(),
        }
    }
}

impl Display for CarrotInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CarrotInstance {{ address: {:?}, version: {} }}",
            self.address, self.version
        )
    }
}

struct Balance {
    coins: Vec<ValuedCoin>,
}
impl Balance {
    fn new(coins: Vec<ValuedCoin>) -> Self {
        Self { coins }
    }
    fn calculate_usd_value(self) -> Uint128 {
        self.coins.iter().fold(Uint128::zero(), |acc, c| {
            acc + c.coin.amount.checked_mul(c.usd_value).unwrap()
        })
    }
}
struct ValuedCoin {
    coin: Coin,
    usd_value: Uint128,
}

impl Scraper {
    pub fn new(
        interchain: DaemonInterchain,
        fetch_cooldown: Duration,
        registry: &Registry,
        chain_ids: Vec<String>,
    ) -> Self {
        let metrics = Metrics::new(registry);

        Self {
            fetch_cooldown,
            interchain,
            last_fetch: SystemTime::UNIX_EPOCH,
            metrics,
            proxy_instances: HashMap::from_iter(chain_ids.into_iter().map(|id| (id, vec![]))),
        }
    }

    // Fetches contracts and assets if fetch cooldown passed
    pub fn scrape(&mut self) -> anyhow::Result<()> {
        // Don't fetch if not ready
        let ready_time = self.last_fetch + self.fetch_cooldown;
        if SystemTime::now() < ready_time {
            return Ok(());
        }

        log!(Level::Info, "Fetching contracts and assets");

        let interchain = &self.interchain;

        // TODO: chain iterator for interchain?
        for (chain_id, proxy_instances) in self.proxy_instances.iter_mut() {
            // TODO: proxy_instances
            *proxy_instances = vec![];
        }
        // let abstr = AbstractClient::new(self.daemon.clone())?;
        // self.proxy_instances[chain_id] = vec![];

        let mut fetch_instances_count = 0;

        // Metrics
        self.metrics.fetch_count.inc();
        self.metrics
            .fetch_instances_count
            .set(fetch_instances_count as i64);

        Ok(())
    }
}

mod utils {
    use cosmos_sdk_proto::{
        cosmos::base::query::v1beta1::{PageRequest, PageResponse},
        cosmwasm::wasm::v1::QueryContractsByCodeResponse,
    };
    use cw_asset::AssetBase;

    use super::*;
    const MIN_REWARD: (&str, Uint128) = ("uosmo", Uint128::new(100_000));

    pub fn next_page_request(page_response: PageResponse) -> PageRequest {
        PageRequest {
            key: page_response.next_key,
            offset: 0,
            limit: 0,
            count_total: false,
            reverse: false,
        }
    }

    /// Get the contract instances of a given code_id
    pub async fn fetch_instances(
        channel: Channel,
        code_id: u64,
        version: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut cw_querier = QueryClient::new(channel);

        let mut contract_addrs = vec![];
        let mut pagination = None;

        loop {
            let QueryContractsByCodeResponse {
                mut contracts,
                pagination: next_pagination,
            } = cw_querier
                .contracts_by_code(QueryContractsByCodeRequest {
                    code_id,
                    pagination,
                })
                .await?
                .into_inner();

            contract_addrs.append(&mut contracts);
            match next_pagination {
                // `next_key` can still be empty, meaning there are no next key
                Some(page_response) if !page_response.next_key.is_empty() => {
                    pagination = Some(next_page_request(page_response))
                }
                // Done with pagination can return out all of the contracts
                _ => {
                    log!(Level::Info, "Savings addrs({version}): {contract_addrs:?}");
                    break anyhow::Ok(contract_addrs);
                }
            }
        }
    }

    /// gets the balance managed by an instance
    pub fn get_proxy_balance(
        daemon: Daemon,
        assets_values: &HashMap<AssetInfo, Uint128>,
        contract_addr: &Addr,
    ) -> anyhow::Result<Uint128> {
        // TODO: get proxy balance to summarize TVL
        let balance = Balance::new(vec![]);
        let balance = balance.calculate_usd_value();
        log!(
            Level::Info,
            "contract: {contract_addr:?} balance: {balance:?}"
        );
        Ok(balance)
    }

    pub fn enough_rewards(rewards: AssetBase<String>) -> bool {
        let gas_asset = match rewards.info {
            cw_asset::AssetInfoBase::Native(denom) => denom == MIN_REWARD.0,
            _ => false,
        };
        gas_asset && rewards.amount >= MIN_REWARD.1
    }
}
