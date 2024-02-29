use std::time::Duration;

use serde::{Deserialize, Serialize};

// todo better layer separation
#[tonic::async_trait]
pub trait BaseTask {
	// todo offset and limit are tmp args for POC, remove after
	async fn run(&mut self, offset: Option<u64>, limit: Option<u64>) -> Vec<TaskResponse>;

	fn get_sleep_interval(&self) -> Duration;

	fn get_state(&self) -> BaseTaskState;

	// get job id
	fn get_id(&self) -> String;

	// if job finished
	fn get_is_finished(&self) -> bool;

	// get serialized state to store to a db
	fn get_state_dump(&self) -> String;
}

// todo, dublicate for proto struct, remove once settled
// todo rename TaskRecord
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskResponse {
	pub id: usize,
	pub timestamp: String,
	pub job_id: String,
	pub data: String,
	pub schema_id: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseTaskState {
	pub is_finished: bool,
	pub is_synced: bool,
	pub records_total: usize,
	// last_update
}
