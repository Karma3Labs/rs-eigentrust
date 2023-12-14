use std::time::Duration;

#[tonic::async_trait]
pub trait BaseTask { 
    async fn run(&mut self);

    async fn normalize(&self);

    fn get_sleep_interval(&self) -> Duration;

    fn get_is_synced(&self) -> bool;

    // get job id
    fn get_id(&self) -> String;

    // get serialized state to store to a db
    fn get_state_dump(&self) -> String;
}
