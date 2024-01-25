use digest::Digest;
use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use sha3::Sha3_256;
use std::time::Duration;
use tracing::{debug, info};

pub use crate::clients::metamask_connector::client::MetamaskConnectorClient;
pub use crate::tasks::types::{BaseTaskState, TaskRecord, TaskTrait};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetamaskConnectorTaskState {
	from: u64,
	range: u64,
	global: BaseTaskState,
}

pub struct MetamaskConnectorTask {
	client: MetamaskConnectorClient,
	state: MetamaskConnectorTaskState,
}

const DEFAULT_SLEEP_INTERVAL_SECONDS: u64 = 5;

impl MetamaskConnectorTask {
	pub fn new(client: MetamaskConnectorClient) -> Self {
		let global = BaseTaskState { is_synced: false, is_finished: false, records_total: 0 };
		let state = MetamaskConnectorTaskState { from: 0, range: 2000, global };

		debug!("Metamask connector task created");
		MetamaskConnectorTask { client, state }
	}

	fn update_state(&mut self, new_state: MetamaskConnectorTaskState) {
		self.state = new_state;
	}
}

#[tonic::async_trait]
impl TaskTrait for MetamaskConnectorTask {
	async fn run(&mut self, offset: Option<u64>, limit: Option<u64>) -> Vec<TaskRecord> {
		let from = offset.unwrap_or(self.state.from);
		let range = limit.unwrap_or(self.state.from + self.state.range);

		info!("Fetching records [{}..{}] lines", from, range);
		let records = self.client.query(Some(from), Some(range)).await.unwrap();

		let records_total = records.len();
		info!("Received {:?} records", records_total);

		let results: Vec<TaskRecord> = records
			.into_iter()
			.enumerate()
			.map(|(i, record)| -> TaskRecord {
				let r = record;

				TaskRecord {
					timestamp: "0".to_string(),
					id: from as usize + i,
					job_id: "0".to_string(),
					schema_id: 0,
					data: r.clone(),
				}
			})
			.collect();

		let from_new = self.state.from + (records_total as u64);
		let records_total_new = self.state.global.records_total + records_total;

		let global =
			BaseTaskState { is_synced: true, is_finished: false, records_total: records_total_new };

		let new_state = MetamaskConnectorTaskState { from: from_new, global, ..self.state };
		self.update_state(new_state);

		results
	}

	fn get_sleep_interval(&self) -> Duration {
		let duration = Duration::from_secs(DEFAULT_SLEEP_INTERVAL_SECONDS);
		duration
	}

	fn get_id(&self) -> String {
		let data = format!("{}", self.client.config.url);
		let mut hasher = Sha3_256::new();
		hasher.update(data.as_bytes());
		let byte_vector = hasher.finalize().to_vec();
		let hash = hex::encode(&byte_vector);

		let id = format!("{}{}", "metamask-connector:", hash);
		id
	}

	fn get_state(&self) -> BaseTaskState {
		self.state.global.clone()
	}

	fn get_is_finished(&self) -> bool {
		self.state.global.is_finished
	}

	fn get_state_dump(&self) -> String {
		let json_string = serde_json::to_string(&self.state).expect("Failed to serialize to JSON");
		json_string
	}

	fn set_state_dump(&mut self, state_json_string: &str) {
		let state: MetamaskConnectorTaskState = serde_json::from_str(state_json_string).unwrap();
		self.update_state(state);
	}
}
