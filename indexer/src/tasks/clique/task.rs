use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use tracing::{ info, Level };
use tokio::task::block_in_place;

// todo higher level interface
pub use crate::clients::clique::client::{ CliqueClient };
pub use crate::tasks::types::{ TaskBase };
use crate::config::EVMIndexerConfig;

pub struct CliqueTask {
    config: EVMIndexerConfig,
    client: CliqueClient,
}

impl CliqueTask {
    pub fn new(config: EVMIndexerConfig, client: CliqueClient) -> Self {
        // todo debug!
        info!("Clique task created");
        CliqueTask { config, client }
    }

    async fn query(&self) {

    }
}

impl TaskBase for CliqueTask {
     fn run(&self) {
        let get_logs = || { self.client.query(None, None) };
        let logs = block_in_place(|| tokio::runtime::Runtime::new().unwrap().block_on(get_logs()));

        for log in logs {
            info!("Log: {:?}", log);
        }
    }

    fn normalize(&self) {}
}