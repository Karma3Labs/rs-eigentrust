use web3::transports::Http;
use web3::types::{ Block, FilterBuilder };
use crate::config::EVMIndexerConfig;
use tracing::{ info, Level };

// todo higher level interface
pub use crate::clients::clique::client::{ CliqueClient };

pub struct CliqueTask {
    config: EVMIndexerConfig,
    client: CliqueClient,
}

impl CliqueTask {
    pub fn new(config: EVMIndexerConfig, client: CliqueClient) -> Self {
        // todo debug!
        info!("Clique client created");
        CliqueTask { config, client }
    }

    pub async fn run(&self) {
        let logs = self.client.query().await;

        for log in logs {
            info!("Log: {:?}", log);
        }
    }
}
