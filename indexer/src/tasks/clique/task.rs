use web3::transports::Http;
use web3::types::{ Block, FilterBuilder, Log };
use tracing::{ info, Level };
use std::cell::RefCell;
use tokio::task;

pub use crate::clients::types::{ EVMLogsClient };
// todo change to EVMLogsClient threadsafe
pub use crate::clients::clique::client::{ CliqueClient };

pub use crate::tasks::types::{ TaskBase };
use crate::config::EVMIndexerConfig;

#[derive(Clone)]
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
        // init prev state
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

    fn update_state(&self, new_state: CliqueTaskState) {
        // let mut state = self.state.borrow_mut();
        // *state = new_state;
    }

    fn run_sync(&self) {

    }
}

#[tonic::async_trait]
impl TaskBase for CliqueTask {
    async fn run(&self) {
        info!(
            "Indexing logs in [{},{}]",
            self.state.from_block,
            self.state.from_block + self.state.range
        );

        let logs = self.client.query(Some(self.state.from_block), Some(self.state.range)).await;

        info!("Found {:?} logs", logs.len());

        // todo
        /*let new_from_block = state.from_block + state.range;
        let new_state = CliqueTaskState {
            from_block: new_from_block,
            range: state.range,
        };*/
    }

    async fn normalize(&self) {}
}
