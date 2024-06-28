use cw_orch::{
    daemon::{Daemon, DaemonError},
    environment::ChainInfo,
};

pub struct ScrapingChains(Vec<ChainInfo>);

impl ScrapingChains {
    pub fn new(chain_infos: Vec<ChainInfo>) -> Self {
        Self(chain_infos)
    }

    pub fn iter(&self) -> ScrapingChainsIterator {
        ScrapingChainsIterator {
            chains: self,
            index: 0,
        }
    }

    pub fn chain_ids(&self) -> Vec<String> {
        self.0.iter().map(|c| c.chain_id.to_owned()).collect()
    }
}

pub struct ScrapingChainsIterator<'a> {
    chains: &'a ScrapingChains,
    index: usize,
}

impl<'a> Iterator for ScrapingChainsIterator<'a> {
    type Item = Result<Daemon, DaemonError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(chain) = self.chains.0.get(self.index) {
            self.index += 1;
            Some(Daemon::builder().chain(chain.clone()).build())
        } else {
            None
        }
    }
}
