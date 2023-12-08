use std::time::Duration;

#[tonic::async_trait]
pub trait TaskBase {
    // 
    async fn run(&mut self);

    //
    async fn normalize(&self);

    fn get_sleep_interval(&self) -> Duration;

    fn get_id(&self) -> String;
}
