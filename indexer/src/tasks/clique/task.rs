use tracing::{ info, debug };

use std::time::Duration;

use digest::Digest;
use sha3::Sha3_256;
use hex;
use serde::{ Serialize, Deserialize };
use serde_json;

pub use crate::clients::types::{ EVMLogsClient };
// todo change to EVMLogsClient, make threadsafe
pub use crate::clients::clique::client::{ CliqueClient };
pub use crate::clients::clique::types::{ EVMIndexerConfig };

pub use crate::tasks::types::{ BaseTask };

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliqueTaskState {
    from_block: u64,
    range: u64,
    is_synced: bool,
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
        let is_synced = false;

        let state = CliqueTaskState {
            from_block,
            range,
            is_synced,
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
impl BaseTask for CliqueTask {
    async fn run(&mut self) {
        info!(
            "Indexing logs in [{}..{}] block range",
            self.state.from_block,
            self.state.from_block + self.state.range - 1
        );

        // todo
        let _ = self.client.query(Some(self.state.from_block), Some(self.state.range)).await;

        let mut logs = Vec::new();
        logs.push(String::from("Hello"));

        if logs.len() > 0 {
            info!("Found {:?} log records", logs.len());
        }

        // todo set to actual last synced block
        let from_block_new = self.state.from_block + self.state.range;
        let new_state = CliqueTaskState {
            from_block: from_block_new,
            ..self.state
        };

        self.update_state(new_state);
    }

    async fn normalize(&self) {}

    fn get_sleep_interval(&self) -> Duration {
        // todo interval if reaches the latest onchain block
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

    fn get_is_synced(&self) -> bool {
        self.state.is_synced
    }

    fn get_state_dump(&self) -> String {
        let json_string = serde_json::to_string(&self.state).expect("Failed to serialize to JSON");
        json_string
    }
}
