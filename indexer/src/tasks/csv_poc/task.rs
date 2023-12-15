use tracing::{ info, debug };

use std::time::Duration;

use digest::Digest;
use sha3::Sha3_256;
use hex;
use serde::{ Serialize, Deserialize };
use serde_json;

pub use crate::clients::types::{ EVMLogsClient };
// todo change to EVMLogsClient, make threadsafe
pub use crate::clients::csv::client::{ CSVClient };

pub use crate::tasks::types::{ BaseTask, BaseTaskState };

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

impl CSVPOCTask {
    pub fn new(client: CSVClient) -> Self {
        // todo restore prev state
        let from = 0;
        let range = 1000;
        let is_synced = false;
        let is_finished = false;

        let global = BaseTaskState {
            is_synced,
            is_finished
        };

        let state = CSVPOCTaskState {
            from,
            range,
            global,
        };

        debug!("CSV POC task created");
        CSVPOCTask {
            client,
            state,
        }
    }

    fn update_state(&mut self, new_state: CSVPOCTaskState) {
        self.state = new_state;
    }
}

#[tonic::async_trait]
impl BaseTask for CSVPOCTask {
    async fn run(&mut self) {
        info!(
            "Parsing CSV [{}..{}] lines",
            self.state.from,
            self.state.from + self.state.range - 1
        );

        let res = self.client.query(Some(self.state.from), Some(self.state.range)).await.unwrap();

        info!("Received {:?} records", res.len());
        let is_finished = self.state.range > res.len().try_into().unwrap();

        let global = BaseTaskState {
            is_synced: is_finished,
            is_finished
        };

        let from_new = self.state.from + self.state.range;
        let new_state = CSVPOCTaskState {
            from: from_new,
            global,
            ..self.state
        };

        self.update_state(new_state);
    }

    fn get_sleep_interval(&self) -> Duration {
        let duration = Duration::from_secs(0);
        duration
    }

    // todo filename
    fn get_id(&self) -> String {
        let data = format!("{}", "file");
        let mut hasher = Sha3_256::new();
        hasher.update(data.as_bytes());
        let byte_vector = hasher.finalize().to_vec();
        let hash = hex::encode(&byte_vector);

        let id = format!("{}{}", "csv-poc:", hash);
        id
    }

    fn get_state(&self) -> BaseTaskState {
        self.state.global.clone()
    }

    fn get_state_dump(&self) -> String {
        let json_string = serde_json::to_string(&self.state).expect("Failed to serialize to JSON");
        json_string
    }
}
