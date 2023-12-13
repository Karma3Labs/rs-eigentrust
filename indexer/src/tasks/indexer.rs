use web3::transports::Http;
use web3::types::{ Block, FilterBuilder };
use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };

// todo redundant layer
// todo higher level interface
pub use crate::clients::clique::client::{ CliqueClient };

pub struct Indexer {
    task: CliqueClient,
}

impl Indexer {
    pub fn new(task: CliqueClient) -> Self {
        // todo debug!
        info!("Indexer created");
        Indexer { client }
    }

    pub async fn run(&self) {
        
    }
}
