use tracing::{ info, debug, Level };
use tokio::task;
use std::time::Duration;
use digest::Digest;
use sha3::Sha3_256;
use hex;

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
        // todo restore prev state
        let from_block = config.from_block;
        let range = 100;

        let state = CliqueTaskState {
            from_block,
            range,
        };

        debug!("Clique task created");
        CliqueTask {
            config,
            client,
            state,
        }
    }

    fn update_state(&mut self, new_state: CliqueTaskState) {
        self.state = new_state;
    }
}

#[tonic::async_trait]
impl TaskBase for CliqueTask {
    async fn run(&mut self) {
        // todo
        let _ = self.client.query(Some(self.state.from_block), Some(self.state.range)).await;

        let mut logs = Vec::new();
        logs.push(String::from("Hello"));

        if logs.len() > 0 {
            info!("Found {:?} log records", logs.len());
        }

        let new_from_block = self.state.from_block + self.state.range;
        let new_state = CliqueTaskState {
            from_block: new_from_block,
            ..self.state
        };

        self.update_state(new_state);
    }

    async fn normalize(&self) {}

    fn get_sleep_interval(&self) -> Duration {
        // todo interval if reaches actual block
        let duration = Duration::from_secs(0);
        duration
    }

    // todo use chain id instead of rpc url
    fn get_id(&self) -> String {
        let data = format!("{}{}", self.config.rpc_url, self.config.master_registry_contract);
        let mut hasher = Sha3_256::new();
        hasher.update(data.as_bytes());
        let byte_vector = hasher.finalize().to_vec();
        let hash = hex::encode(&byte_vector);

        let id = format!("{}{}", "clique:", hash);
        id
    }
}
