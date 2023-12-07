use web3::types::{ Log };
use std::{ error::Error };

#[tonic::async_trait]
pub trait EVMLogsClient {
    async fn query(
        &self,
        from: Option<u64>,
        range: Option<u64>
    ) -> Vec<Log>;
}
