// todo generic Result
use heed::{ Result };
use heed::types::*;

// #[tonic::async_trait]
pub trait BaseKVStorage {
    fn put(&self, key: &str, value: &str) -> Result<()>;

    fn get(&self, key: &str) -> Option<String>;
}
