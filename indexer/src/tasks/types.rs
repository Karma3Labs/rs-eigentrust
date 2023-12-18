use std::time::Duration;
use serde::{ Serialize, Deserialize };

#[tonic::async_trait]
pub trait BaseTask { 
    async fn run(&mut self);

    fn get_sleep_interval(&self) -> Duration;

    fn get_state(&self) -> BaseTaskState;

    // get job id
    fn get_id(&self) -> String;

    // if job finished
    fn get_is_finished(&self) -> bool;

    // get serialized state to store to a db
    fn get_state_dump(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseTaskState {
    pub is_finished: bool,
    pub is_synced: bool,
    // last_update
}
