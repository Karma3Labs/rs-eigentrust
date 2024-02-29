use std::time::Duration;

use digest::Digest;
use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use sha3::Sha3_256;
use tracing::{debug, info};

use crate::clients::csv::client::CSVClient;
use crate::tasks::types::{BaseTask, BaseTaskState, TaskResponse};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CSVPOCTaskState {
	from: u64,
	range: u64,
	global: BaseTaskState,
}

pub struct CSVPOCTask {
	client: CSVClient,
	state: CSVPOCTaskState,
}

const CSV_COLUMN_INDEX_DATA: usize = 3;
const CSV_COLUMN_SCHEMA_ID: usize = 2;
const CSV_COLUMN_INDEX_TIMESTAMP: usize = 1;
const CSV_COLUMN_INDEX: usize = 0;

impl CSVPOCTask {
	pub fn new(client: CSVClient) -> Self {
		let global = BaseTaskState { is_synced: false, is_finished: false, records_total: 0 };

		// todo restore prev state
		let state = CSVPOCTaskState { from: 0, range: 2000, global };

		debug!("CSV POC task created");
		CSVPOCTask { client, state }
	}

	fn update_state(&mut self, new_state: CSVPOCTaskState) {
		self.state = new_state;
	}
}

#[tonic::async_trait]
impl BaseTask for CSVPOCTask {
	async fn run(&mut self, offset: Option<u64>, limit: Option<u64>) -> Vec<TaskResponse> {
		let from = offset.unwrap_or(self.state.from);
		let range = limit.unwrap_or(self.state.from + self.state.range);

		info!("Parsing CSV [{}..{}] lines", from, range);

		let records = self.client.query(Some(from), Some(range)).await.unwrap();

		let records_total = records.len();
		info!("Received {:?} records", records_total);

		let is_finished = self.state.range > records_total.try_into().unwrap();

		let results: Vec<TaskResponse> = records
			.into_iter()
			.map(|record| -> TaskResponse {
				let r = record.unwrap();

				let schema_id = r.get(CSV_COLUMN_SCHEMA_ID).unwrap().parse::<usize>().unwrap_or(0);
				TaskResponse {
					timestamp: r.get(CSV_COLUMN_INDEX_TIMESTAMP).unwrap().to_string(),
					id: r.get(CSV_COLUMN_INDEX).unwrap().parse::<usize>().unwrap_or(0),
					job_id: "0".to_string(),
					schema_id,
					data: r.get(CSV_COLUMN_INDEX_DATA).unwrap().to_string(),
				}
			})
			.collect();

		let global = BaseTaskState { is_synced: is_finished, is_finished, records_total };

		let _from_new = self.state.from + self.state.range;
		let new_state = CSVPOCTaskState {
			// from: _from_new,
			global,
			..self.state
		};

		self.update_state(new_state);

		results
	}

	fn get_sleep_interval(&self) -> Duration {
		Duration::from_secs(0)
	}

	fn get_id(&self) -> String {
		// todo filename
		let data = "file".to_string();
		let mut hasher = Sha3_256::new();
		hasher.update(data.as_bytes());
		let byte_vector = hasher.finalize().to_vec();
		let hash = hex::encode(byte_vector);

		format!("csv-poc:{}", hash)
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
