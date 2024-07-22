use std::collections::HashMap;

use crate::{abstract_daemon_state::AbstractDaemonState, contract_state::ContractState};

use super::utils;
use abstract_std::{
    objects::{module::ModuleInfo, module_reference::ModuleReference, AccountId},
    version_control::AccountBase,
    VERSION_CONTROL,
};
use cw_orch::{
    daemon::{senders::QueryOnlyDaemon, RUNTIME},
    prelude::*,
};

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

#[derive(Default)]
pub struct ScrapedData {
    // proxy local instances
    pub account_local_instances: Vec<AccountInstance>,
    // proxy remote instances
    pub account_remote_instances: Vec<AccountInstance>,
    // Namespace -> Modules
    pub modules_by_namespace: HashMap<String, Vec<(ModuleInfo, ModuleReference)>>,
}

impl ScrapedData {
    pub fn scrape_data(daemon: &QueryOnlyDaemon, abstract_state: &AbstractDaemonState) -> Self {
        let version_control_addr =
            abstract_state.contract_addr(&daemon.env_info(), VERSION_CONTROL);

        // Load version control state
        let version_control_state = RUNTIME
            .handle()
            .block_on(utils::fetch_contract_state(
                daemon.channel(),
                version_control_addr,
            ))
            .unwrap_or_default();

        let (account_local_instances, account_remote_instances) =
            Self::account_instances(&version_control_state);

        let modules_by_namespace = Self::modules_by_namespace(&version_control_state);

        Self {
            account_local_instances,
            account_remote_instances,
            modules_by_namespace,
        }
    }

    fn account_instances(
        version_control_state: &ContractState,
    ) -> (Vec<AccountInstance>, Vec<AccountInstance>) {
        let mut local_instances = vec![];
        let mut remote_instances = vec![];
        // Sort all accounts
        for (account_id, base) in abstract_std::version_control::state::ACCOUNT_ADDRESSES
            .range(
                version_control_state,
                None,
                None,
                cosmwasm_std::Order::Ascending,
            )
            .flatten()
        {
            log::debug!("Saving account base: {base:?} for {account_id}");
            if account_id.is_local() {
                local_instances.push(AccountInstance::new(account_id, base));
            } else {
                remote_instances.push(AccountInstance::new(account_id, base));
            }
        }
        (local_instances, remote_instances)
    }

    fn modules_by_namespace(
        version_control_state: &ContractState,
    ) -> HashMap<String, Vec<(ModuleInfo, ModuleReference)>> {
        let mut modules_by_namespace: HashMap<String, Vec<(ModuleInfo, ModuleReference)>> =
            HashMap::new();
        for (module_info, module_reference) in
            abstract_std::version_control::state::REGISTERED_MODULES
                .range(
                    version_control_state,
                    None,
                    None,
                    cosmwasm_std::Order::Ascending,
                )
                .flatten()
        {
            let modules = modules_by_namespace
                .entry(module_info.namespace.to_string())
                .or_insert(vec![]);
            modules.push((module_info, module_reference));
        }
        modules_by_namespace
    }
}
