use digest::Digest;
use hex;
use mm_spd_vc::OneOrMore;
use serde::{Deserialize, Serialize};
use serde_json;
use sha3::Sha3_256;
use std::time::Duration;
use tracing::{debug, info};

pub use crate::clients::metamask_connector::client::MetamaskConnectorClient;
pub use crate::clients::metamask_connector::types::MetamaskAPIRecord;

pub use crate::tasks::types::{TaskGlobalState, TaskRecord, TaskTrait};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetamaskConnectorTaskState {
	from: u64,
	range: u64,
	global: TaskGlobalState,
}

pub struct MetamaskConnectorTask {
	client: MetamaskConnectorClient,
	state: MetamaskConnectorTaskState,
}

const DEFAULT_SLEEP_INTERVAL_SECONDS: u64 = 5;

impl MetamaskConnectorTask {
	pub fn new(client: MetamaskConnectorClient) -> Self {
		let global = TaskGlobalState { is_synced: false, is_finished: false, records_total: 0 };
		let state = MetamaskConnectorTaskState { from: 1, range: 2000, global };

		debug!("Metamask connector task created");
		MetamaskConnectorTask { client, state }
	}

	fn update_state(&mut self, new_state: MetamaskConnectorTaskState) {
		self.state = new_state;
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BaseCred {
	#[serde(rename = "type")]
	type_: OneOrMore<String>,
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
			.filter_map(|(_i, record)| -> Option<TaskRecord> {
				let r = record;

				// TODO(ek): This is a hack: this code path is supposed to be content-agnostic.
				let type_ = match serde_json::from_value::<BaseCred>(r.assertion.clone()) {
					Ok(cred) => cred.type_,
					Err(err) => {
						info!(?err, ?r.assertion, "assertion doesn't seem to be a VC");
						return None;
					},
				};
				let schema_id = if type_.matches("SecurityReportCredential") {
					0
				} else if type_.matches("ReviewCredential") {
					1
				} else if type_.matches("TrustCredential") {
					2
				} else {
					info!(?type_, ?r.assertion, "invalid VC type");
					return None;
				};
				// info!(?type_, schema_id, "matched VC type");

				let timestamp = match time::PrimitiveDateTime::parse(
					&r.creation_at,
					&time::format_description::well_known::Iso8601::DEFAULT,
				) {
					Ok(timestamp) => timestamp,
					Err(err) => {
						info!(?err, ?r.assertion, "cannot parse acceptance timestamp");
						return None;
					},
				}
				.assume_utc()
				.unix_timestamp_nanos();
				let timestamp = (timestamp / 1_000_000).to_string();
				Some(TaskRecord {
					timestamp,
					id: r.id,
					job_id: "0".to_string(),
					schema_id,
					data: r.assertion.to_string(),
				})
			})
			.collect();

		let from_new = self.state.from + (records_total as u64);
		let records_total_new = self.state.global.records_total + records_total;

		let global = TaskGlobalState {
			is_synced: true,
			is_finished: false,
			records_total: records_total_new,
		};

		let new_state = MetamaskConnectorTaskState { from: from_new, global, ..self.state };
		self.update_state(new_state);

		results
	}

	fn get_sleep_interval(&self) -> Duration {
		// todo 0 if not synced
		Duration::from_secs(DEFAULT_SLEEP_INTERVAL_SECONDS)
	}

	fn get_id(&self) -> String {
		let data = self.client.config.url.to_string();
		let mut hasher = Sha3_256::new();
		hasher.update(data.as_bytes());
		let byte_vector = hasher.finalize().to_vec();
		let hash = hex::encode(byte_vector);

		let id = format!("{}{}", "metamask-connector:", hash);
		id
	}

	fn get_state(&self) -> TaskGlobalState {
		self.state.global.clone()
	}

	fn get_is_finished(&self) -> bool {
		self.state.global.is_finished
	}

	fn get_state_dump(&self) -> String {
		serde_json::to_string(&self.state).expect("Failed to serialize to JSON")
	}

	fn set_state_dump(&mut self, state_json_string: &str) {
		let state: MetamaskConnectorTaskState = serde_json::from_str(state_json_string).unwrap();
		self.update_state(state);
	}
}
