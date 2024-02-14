use crate::storage::types::BaseKVStorage;

#[derive(Default)]
pub struct MockDBClient;

#[allow(unused)]
impl MockDBClient {
	pub fn new() -> Self {
		MockDBClient {}
	}
}

impl BaseKVStorage for MockDBClient {
	fn put(&self, _key: &str, _value: &str) -> heed::Result<()> {
		Ok(())
	}

	fn get(&self, _key: &str) -> Option<String> {
		Some("Mock db response".to_string())
	}
}
