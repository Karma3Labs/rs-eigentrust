use std::time::Duration;

use serde::{Deserialize, Serialize};

// todo better layer separation rename to TaskTrait
#[tonic::async_trait]
pub trait TaskTrait {
	// todo offset and limit are tmp args for POC, remove after
	async fn run(&mut self, offset: Option<u64>, limit: Option<u64>) -> Vec<TaskRecord>;

	fn get_sleep_interval(&self) -> Duration;

	fn get_state(&self) -> TaskGlobalState;

	// get job id, move hashing logic to utils
	fn get_id(&self) -> String;

	// if job finished
	fn get_is_finished(&self) -> bool;

	// get serialized state to store to a db
	fn get_state_dump(&self) -> String;

	// deserialize and set state
	fn set_state_dump(&mut self, state_json_string: &str);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRecord {
	pub id: usize,
	pub timestamp: String,
	pub job_id: String,
	pub data: String,
	pub schema_id: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskGlobalState {
	pub is_finished: bool,
	pub is_synced: bool,
	pub records_total: usize,
	// last_update
}
