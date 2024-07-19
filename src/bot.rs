use crate::{abstract_state::AbstractState, Metrics, ScrapingChains};
use abstract_client::{AbstractClient, AccountSource, Environment};
use abstract_interface::{Abstract, Proxy};
use abstract_std::{
    objects::AccountId, proxy::state::ACCOUNT_ID, version_control::AccountBase, PROXY,
    VERSION_CONTROL,
};
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
    account_local_instances: HashMap<String, Vec<AccountInstance>>,
    // proxy remote instances
    // Chain id -> (account id, proxy addresses)
    account_remote_instances: HashMap<String, Vec<AccountInstance>>,
}

#[derive(Clone, Debug)]
pub struct AccountInstance {
    pub account_id: AccountId,
    pub base: AccountBase,
}

impl AccountInstance {
    pub fn new(account_id: AccountId, base: AccountBase) -> Self {
        Self { account_id, base }
    }
}

// #[derive(Clone, Debug)]
// pub struct ProxyInstance {
//     pub account_id: AccountId,
//     pub addr: String,
// }

// impl ProxyInstance {
//     pub fn new(account_id: AccountId, addr: String) -> Self {
//         Self { account_id, addr }
//     }
// }

// struct Balance {
//     coins: Vec<ValuedCoin>,
// }
// impl Balance {
//     fn new(coins: Vec<ValuedCoin>) -> Self {
//         Self { coins }
//     }
//     fn calculate_usd_value(self) -> Uint128 {
//         self.coins.iter().fold(Uint128::zero(), |acc, c| {
//             acc + c.coin.amount.checked_mul(c.usd_value).unwrap()
//         })
//     }
// }
// struct ValuedCoin {
//     coin: Coin,
//     usd_value: Uint128,
// }

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
            account_local_instances: Default::default(),
            account_remote_instances: Default::default(),
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
                    let env_info = daemon.env_info();
                    let abstr = AbstractClient::new(daemon.clone())?;

                    // let proxy_code_id = self.abstract_state.contract_code_id(&env_info, PROXY);
                    let chain_id = env_info.chain_id;
                    // Save account instances for chain
                    let (account_local_instances, account_remote_instances) =
                        account_instances(daemon.channel(), abstr.version_control().address()?);
                    self.account_local_instances
                        .insert(chain_id.clone(), account_local_instances);
                    self.account_remote_instances
                        .insert(chain_id.clone(), account_remote_instances);
                    dbg!(&self.account_local_instances[&chain_id].len());
                    dbg!(&self.account_remote_instances[&chain_id].len());
                }
                Err(e) => {
                    log::error!("{e}");
                }
            }
        }

        // Metrics
        self.update_metrics();

        Ok(())
    }

    fn update_metrics(&mut self) {
        self.metrics.fetch_count.inc();

        for (chain_id, accounts) in self.account_local_instances.iter() {
            let label = labels! {"chain_id" => chain_id.as_str()};
            self.metrics
                .local_account_instances_count
                .with(&label)
                .set(accounts.len() as u64);
        }
        for (chain_id, accounts) in self.account_remote_instances.iter() {
            let label = labels! {"chain_id" => chain_id.as_str()};
            self.metrics
                .remote_account_instances_count
                .with(&label)
                .set(accounts.len() as u64);
        }
    }
}

// fn proxy_instances(
//     channel: Channel,
//     proxy_code_id: u64,
// ) -> (Vec<ProxyInstance>, Vec<ProxyInstance>) {
//     let mut proxy_local_instances = vec![];
//     let mut proxy_remote_instances = vec![];

//     // Load proxy addresses
//     let proxy_addrs = RUNTIME
//         .handle()
//         .block_on(utils::fetch_instances(channel.clone(), proxy_code_id))
//         .unwrap_or_default();

//     // Get all addrs
//     let mut client: QueryClient<Channel> = QueryClient::new(channel);
//     for proxy_addr in proxy_addrs {
//         if let Ok(response) =
//             RUNTIME.block_on(client.raw_contract_state(QueryRawContractStateRequest {
//                 address: proxy_addr.clone(),
//                 query_data: ACCOUNT_ID.as_slice().to_owned(),
//             }))
//         {
//             let account_id: AccountId = from_json(response.into_inner().data).unwrap();
//             log::debug!("Saving proxy addr: {proxy_addr} for {account_id}");
//             if account_id.is_local() {
//                 proxy_local_instances.push(ProxyInstance::new(account_id, proxy_addr));
//             } else {
//                 proxy_remote_instances.push(ProxyInstance::new(account_id, proxy_addr));
//             }
//         }
//     }

//     (proxy_local_instances, proxy_remote_instances)
// }

fn account_instances(
    channel: Channel,
    version_control_addr: Addr,
) -> (Vec<AccountInstance>, Vec<AccountInstance>) {
    let mut local_instances = vec![];
    let mut remote_instances = vec![];

    // Load proxy addresses
    let version_control_state = RUNTIME
        .handle()
        .block_on(utils::fetch_contract_state(
            channel.clone(),
            version_control_addr,
        ))
        .unwrap_or_default();

    // Sort all accounts
    for account in abstract_std::version_control::state::ACCOUNT_ADDRESSES.range(
        &version_control_state,
        None,
        None,
        cosmwasm_std::Order::Ascending,
    ) {
        if let Ok((account_id, base)) = account {
            log::debug!("Saving account base: {base:?} for {account_id}");
            if account_id.is_local() {
                local_instances.push(AccountInstance::new(account_id, base));
            } else {
                remote_instances.push(AccountInstance::new(account_id, base));
            }
        }
    }

    (local_instances, remote_instances)
}

mod utils {
    use cosmos_sdk_proto::{
        cosmos::base::query::v1beta1::{PageRequest, PageResponse},
        cosmwasm::wasm::v1::{
            QueryAllContractStateRequest, QueryAllContractStateResponse,
            QueryContractsByCodeResponse,
        },
    };

    use crate::contract_state::ContractState;

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
        let mut page_number = 0;
        loop {
            log::debug!(
                "Fetching instances of {code_id}, page[{page_number}] key: {page_key:?}",
                page_key = pagination
                    .as_ref()
                    .map(|p: &PageRequest| String::from_utf8_lossy(&p.key))
                    .unwrap_or_default()
            );
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
                    page_number += 1;
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

    /// Get the contract state of a given contract addr
    pub async fn fetch_contract_state(
        channel: Channel,
        contract_addr: Addr,
    ) -> anyhow::Result<ContractState> {
        let mut cw_querier = QueryClient::new(channel);

        let mut contract_models = vec![];
        let mut pagination = None;
        let mut page_number = 0;
        loop {
            log::debug!(
                "Fetching instances of accounts, page[{page_number}] key: {page_key:?}",
                page_key = pagination
                    .as_ref()
                    .map(|p: &PageRequest| String::from_utf8_lossy(&p.key))
                    .unwrap_or_default()
            );
            let QueryAllContractStateResponse {
                models,
                pagination: next_pagination,
            } = cw_querier
                .all_contract_state(QueryAllContractStateRequest {
                    address: contract_addr.to_string(),
                    pagination,
                })
                .await?
                .into_inner();

            contract_models.extend(models);
            match next_pagination {
                // `next_key` can still be empty, meaning there are no next key
                Some(page_response) if !page_response.next_key.is_empty() => {
                    page_number += 1;
                    pagination = Some(next_page_request(page_response))
                }
                // Done with pagination can return out all of the contracts
                _ => {
                    // log!(Level::Info, "Savings states: {contract_models:?}");
                    let state = ContractState::new(contract_models);
                    break anyhow::Ok(state);
                }
            }
        }
    }

    // /// gets the balance managed by an instance
    // pub fn get_proxy_balance(
    //     daemon: Daemon,
    //     assets_values: &HashMap<AssetInfo, Uint128>,
    //     contract_addr: &Addr,
    // ) -> anyhow::Result<Uint128> {
    //     // TODO: get proxy balance to summarize TVL
    //     let balance = Balance::new(vec![]);
    //     let balance = balance.calculate_usd_value();
    //     log!(
    //         Level::Info,
    //         "contract: {contract_addr:?} balance: {balance:?}"
    //     );
    //     Ok(balance)
    // }
}
