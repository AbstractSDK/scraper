use cosmos_sdk_proto::cosmwasm::wasm::v1::{
    query_client::QueryClient, QueryContractsByCodeRequest,
};
use cosmos_sdk_proto::{
    cosmos::base::query::v1beta1::{PageRequest, PageResponse},
    cosmwasm::wasm::v1::{
        QueryAllContractStateRequest, QueryAllContractStateResponse, QueryContractsByCodeResponse,
    },
};
use cosmwasm_std::Addr;
use cw_orch::anyhow;
use log::{log, Level};
use tonic::transport::Channel;

use crate::contract_state::ContractState;

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
#[allow(unused)]
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
