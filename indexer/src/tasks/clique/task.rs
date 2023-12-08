use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use tracing::{ info, Level };
use std::cell::RefCell;
use tokio::task;

pub use crate::clients::types::{ EVMLogsClient };
// todo change to EVMLogsClient, make threadsafe
pub use crate::clients::clique::client::{ CliqueClient };
use std::process;

pub use crate::tasks::types::{ TaskBase };
use crate::config::EVMIndexerConfig;

#[derive(Clone, Debug)]
pub struct CliqueTaskState {
    from_block: u64,
    range: u64,
}

pub struct CliqueTask {
    config: EVMIndexerConfig,
    client: CliqueClient,
    state: CliqueTaskState,
}

impl CliqueTask {
    pub fn new(config: EVMIndexerConfig, client: CliqueClient) -> Self {
        // restore prev state

        println!("{}", config.from_block);
        println!("{}", config.rpc_url);

        let from_block = config.from_block;
        let range = 1024;

        let state = CliqueTaskState {
            from_block,
            range,
        };

        info!("Clique task created");
        CliqueTask {
            config,
            client,
            state,
        }
    }

    async fn query(&self) {}

    fn update_state(&mut self, new_state: CliqueTaskState) {
        self.state = new_state;
    }

    fn run_sync(&self) {}
}

#[tonic::async_trait]
impl TaskBase for CliqueTask {
    async fn run(&mut self) {
        info!(
            "Indexing logs in [{},{}] block range",
            self.state.from_block,
            self.state.from_block + self.state.range
        );

        let logs = self.client.query(Some(self.state.from_block), Some(self.state.range)).await;

        if logs.len() > 0 {
            info!("Found {:?} logs", logs.len());
        }

        // todo
        let new_from_block = self.state.from_block + self.state.range;
        let new_state = CliqueTaskState {
            from_block: new_from_block,
            range: self.state.range,
        };

        self.update_state(new_state);
    }

    async fn normalize(&self) {}
}
