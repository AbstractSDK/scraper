#![allow(unused)]

use cw_orch::environment::EnvironmentInfo;

pub struct AbstractDaemonState(abstract_interface::AbstractDaemonState);

impl Default for AbstractDaemonState {
    fn default() -> Self {
        Self(abstract_interface::AbstractDaemonState::default())
    }
}

impl AbstractDaemonState {
    pub fn contract_addr(
        &self,
        env_info: &EnvironmentInfo,
        contract_id: &str,
    ) -> cosmwasm_std::Addr {
        cosmwasm_std::Addr::unchecked(
            self.0.contract_addr(&env_info.chain_id, contract_id)
                .unwrap(),
        )
    }

    pub fn contract_code_id(&self, env_info: &EnvironmentInfo, contract_id: &str) -> u64 {
        self.0.contract_code_id(&env_info.chain_id, contract_id)
            .unwrap()
    }
}
