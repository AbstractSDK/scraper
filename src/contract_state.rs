#![allow(unused)]
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included, Unbounded};

use cosmos_sdk_proto::cosmwasm::wasm::v1::Model;

#[derive(Default)]
pub struct ContractState {
    state: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl ContractState {
    pub fn new(raw_state: Vec<Model>) -> Self {
        let mut state = BTreeMap::new();
        for Model { key, value } in raw_state {
            state.insert(key, value);
        }
        Self { state }
    }
}

impl cosmwasm_std::Storage for ContractState {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.state.get(key).cloned()
    }

    fn range<'a>(
        &'a self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        // TODO: do we care?
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = cosmwasm_std::Record> + 'a> {
        let start = match start {
            Some(start) => Included(start.to_owned()),
            None => Unbounded,
        };
        let end = match end {
            Some(end) => Excluded(end.to_owned()),
            None => Unbounded,
        };
        let range = self
            .state
            .range((start, end))
            .map(|(k, v)| (k.clone(), v.clone()));
        Box::new(range)
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        unimplemented!("no set")
    }

    fn remove(&mut self, key: &[u8]) {
        unimplemented!("no remove")
    }
}
