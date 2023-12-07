#[tonic::async_trait]
pub trait TaskBase {
    async fn run(&self);
    async fn normalize(&self);
}
