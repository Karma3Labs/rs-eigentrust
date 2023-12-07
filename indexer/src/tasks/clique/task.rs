use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use tracing::{ info, Level };
use tokio::task::block_in_place;


pub use crate::clients::clique::client::{ CliqueClient };
pub use crate::tasks::types::{ TaskBase };
use crate::config::EVMIndexerConfig;

pub struct CliqueTask {
    config: EVMIndexerConfig,
    client: CliqueClient,
}

impl CliqueTask {
    pub fn new(config: EVMIndexerConfig, client: CliqueClient) -> Self {
        
        info!("Clique task created");
        CliqueTask { config, client }
    }

    async fn query(&self) {

    }
}

#[tonic::async_trait]
impl TaskBase for CliqueTask {
     async fn run(&self) {
        let logs = self.client.query(None, None).await;

        for log in logs {
            info!("Log: {:?}", log);
        }
    }

    async fn normalize(&self) {}
}