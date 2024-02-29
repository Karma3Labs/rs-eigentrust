use std::time::Duration;

use digest::Digest;
use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use sha3::Sha3_256;
use tracing::{debug, info};

// todo change to EVMLogsClient, make threadsafe
use crate::clients::clique::client::CliqueClient;
use crate::clients::clique::types::EVMIndexerConfig;
use crate::tasks::types::{BaseTask, BaseTaskState, TaskResponse};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliqueTaskState {
	from_block: u64,
	range: u64,
	global: BaseTaskState,
}

pub struct CliqueTask {
	config: EVMIndexerConfig,
	// todo tmp, remove pub
	pub client: CliqueClient,
	state: CliqueTaskState,
}

impl CliqueTask {
	pub fn new(config: EVMIndexerConfig, client: CliqueClient) -> Self {
		// todo restore prev state
		let from_block = config.from_block;
		let range = 100;

		let global = BaseTaskState { is_synced: false, is_finished: false, records_total: 0 };

		let state = CliqueTaskState { from_block, range, global };

		debug!("Clique task created");
		CliqueTask { config, client, state }
	}

	fn update_state(&mut self, new_state: CliqueTaskState) {
		self.state = new_state;
	}
}

#[tonic::async_trait]
impl BaseTask for CliqueTask {
	async fn run(&mut self, _offset: Option<u64>, _limit: Option<u64>) -> Vec<TaskResponse> {
		info!(
			"Indexing logs in [{}..{}] block range",
			self.state.from_block,
			self.state.from_block + self.state.range - 1
		);

		// todo
		let _ = self.client.query(Some(self.state.from_block), Some(self.state.range)).await;

		let logs = vec![String::from("Hello")];

		if !logs.is_empty() {
			info!("Found {:?} log records", logs.len());
		}

		// todo set to actual last synced block
		let from_block_new = self.state.from_block + self.state.range;

		let new_state = CliqueTaskState {
			from_block: from_block_new,
			global: self.state.global.clone(),
			..self.state
		};

		self.update_state(new_state);

		let res: Vec<TaskResponse> = Vec::new();
		res
	}

	fn get_sleep_interval(&self) -> Duration {
		// todo interval if reaches the latest onchain block
		Duration::from_secs(0)
	}

	// todo use chain id instead of rpc url
	fn get_id(&self) -> String {
		let data = format!(
			"{}{}",
			self.config.rpc_url, self.config.master_registry_contract
		);
		let mut hasher = Sha3_256::new();
		hasher.update(data.as_bytes());
		let byte_vector = hasher.finalize().to_vec();
		let hash = hex::encode(byte_vector);

		let id = format!("{}{}", "clique:", hash);
		id
	}

	fn get_state(&self) -> BaseTaskState {
		self.state.global.clone()
	}

	fn get_is_finished(&self) -> bool {
		self.state.global.is_finished
	}

	fn get_state_dump(&self) -> String {
		serde_json::to_string(&self.state).expect("Failed to serialize to JSON")
	}
}
