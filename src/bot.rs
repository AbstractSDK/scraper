use crate::{abstract_state::AbstractState, Metrics, ScrapingChains};
use abstract_client::{AbstractClient, AccountSource, Environment};
use abstract_interface::{Abstract, Proxy};
use abstract_std::{objects::AccountId, proxy::state::ACCOUNT_ID, PROXY, VERSION_CONTROL};
use cosmos_sdk_proto::cosmwasm::wasm::v1::{
    query_client::QueryClient, QueryContractsByCodeRequest, QueryRawContractStateRequest,
};
use cw_asset::AssetInfo;
use cw_orch_interchain::IbcQueryHandler;
use semver::VersionReq;

use cosmwasm_std::{from_json, Uint128};
use cw_orch::{
    anyhow,
    daemon::{queriers::Authz, Daemon, RUNTIME},
    environment::ChainState,
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
    // Abstract state
    abstract_state: AbstractState,
    last_fetch: SystemTime,
    // Chains to Fetch
    // interchain: DaemonInterchain,
    interchain: ScrapingChains,
    // metrics
    metrics: Metrics,
    // proxy local instances
    // Chain id -> (account id, proxy addresses)
    proxy_local_instances: HashMap<String, Vec<ProxyInstance>>,
    // proxy remote instances
    // Chain id -> (account id, proxy addresses)
    proxy_remote_instances: HashMap<String, Vec<ProxyInstance>>,
}

#[derive(Clone, Debug)]
pub struct ProxyInstance {
    pub account_id: AccountId,
    pub addr: String,
}

impl ProxyInstance {
    pub fn new(account_id: AccountId, addr: String) -> Self {
        Self { account_id, addr }
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
        interchain: ScrapingChains,
        fetch_cooldown: Duration,
        registry: &Registry,
        abstract_state: AbstractState,
    ) -> Self {
        let metrics = Metrics::new(registry);

        Self {
            fetch_cooldown,
            abstract_state,
            last_fetch: SystemTime::UNIX_EPOCH,
            metrics,
            proxy_local_instances: Default::default(),
            proxy_remote_instances: Default::default(),
            interchain,
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

        for daemon_result in interchain.iter() {
            match daemon_result {
                Ok(daemon) => {
                    // Get proxy code id from abstract state
                    let proxy_code_id = {
                        let mut abstr = Abstract::load_from(daemon.clone()).unwrap();
                        // TODO: replace with abstract_state
                        abstr
                            .get_contracts_mut()
                            .iter()
                            .find_map(|contract| {
                                (contract.id() == PROXY).then(|| contract.code_id().unwrap())
                            })
                            .unwrap()
                    };
                    let chain_id = daemon.chain_id();
                    // Save proxy instances for chain
                    let (proxy_local_instances, proxy_remote_instances) =
                        proxy_instances(daemon.channel(), proxy_code_id);
                    self.proxy_local_instances
                        .insert(chain_id.clone(), proxy_local_instances);
                    self.proxy_remote_instances
                        .insert(chain_id, proxy_remote_instances);
                }
                Err(e) => {
                    log::error!("{e}");
                }
            }
        }

        dbg!(&self.proxy_local_instances);
        dbg!(&self.proxy_remote_instances);

        // Metrics
        self.update_metrics();

        Ok(())
    }

    fn update_metrics(&mut self) {
        self.metrics.fetch_count.inc();

        for (chain_id, accounts) in self.proxy_local_instances.iter() {
            let label = labels! {"chain_id" => chain_id.as_str()};
            self.metrics
                .local_account_instances_count
                .with(&label)
                .set(accounts.len() as u64);
        }
        for (chain_id, accounts) in self.proxy_remote_instances.iter() {
            let label = labels! {"chain_id" => chain_id.as_str()};
            self.metrics
                .remote_account_instances_count
                .with(&label)
                .set(accounts.len() as u64);
        }
    }
}

fn proxy_instances(
    channel: Channel,
    proxy_code_id: u64,
) -> (Vec<ProxyInstance>, Vec<ProxyInstance>) {
    let mut proxy_local_instances = vec![];
    let mut proxy_remote_instances = vec![];

    // Load proxy addresses
    let proxy_addrs = RUNTIME
        .handle()
        .block_on(utils::fetch_instances(channel.clone(), proxy_code_id))
        .unwrap_or_default();

    // Get all code ids
    let mut client: QueryClient<Channel> = QueryClient::new(channel);
    for proxy_addr in proxy_addrs {
        if let Ok(response) =
            RUNTIME.block_on(client.raw_contract_state(QueryRawContractStateRequest {
                address: proxy_addr.clone(),
                query_data: ACCOUNT_ID.as_slice().to_owned(),
            }))
        {
            let account_id: AccountId = from_json(response.into_inner().data).unwrap();
            if account_id.is_local() {
                proxy_local_instances.push(ProxyInstance::new(account_id, proxy_addr));
            } else {
                proxy_remote_instances.push(ProxyInstance::new(account_id, proxy_addr));
            }
        }
    }

    (proxy_local_instances, proxy_remote_instances)
}

mod utils {
    use cosmos_sdk_proto::{
        cosmos::base::query::v1beta1::{PageRequest, PageResponse},
        cosmwasm::wasm::v1::QueryContractsByCodeResponse,
    };

    use super::*;

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
    pub async fn fetch_instances(channel: Channel, code_id: u64) -> anyhow::Result<Vec<String>> {
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
                    log!(Level::Info, "Savings addrs: {contract_addrs:?}");
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
}
