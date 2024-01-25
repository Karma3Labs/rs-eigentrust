// todo generic Result
use heed::Result;

// #[tonic::async_trait]
pub trait KVStorageTrait {
	fn put(&self, key: &str, value: &str) -> Result<()>;

	fn get(&self, key: &str) -> Option<String>;
}
