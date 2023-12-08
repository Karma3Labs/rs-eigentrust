#[tonic::async_trait]
pub trait TaskBase {
    // 
    async fn run(&mut self);

    //
    async fn normalize(&self);

    // async fn normalize(&self);
}
