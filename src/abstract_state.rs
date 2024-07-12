use cw_orch::environment::EnvironmentInfo;

pub struct AbstractState(serde_json::Value);

impl Default for AbstractState {
    fn default() -> Self {
        Self(abstract_interface::State::load_state())
    }
}

impl AbstractState {
    pub fn contract_addr(
        &self,
        env_info: &EnvironmentInfo,
        contract_id: &str,
    ) -> cosmwasm_std::Addr {
        cosmwasm_std::Addr::unchecked(
            self.0[&env_info.chain_name][&env_info.chain_id]["default"][contract_id]
                .as_str()
                .unwrap(),
        )
    }

    pub fn contract_code_id(&self, env_info: &EnvironmentInfo, contract_id: &str) -> u64 {
        self.0[&env_info.chain_name][&env_info.chain_id]["code_ids"][contract_id]
            .as_u64()
            .unwrap()
    }
}
