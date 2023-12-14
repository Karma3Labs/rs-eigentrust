use heed::{ EnvOpenOptions, Database, RoTxn, RwTxn, Result };
use heed::types::*;
use crate::storage::types::{ BaseKVStorage };

pub struct MockDBClient;

impl MockDBClient {
    pub fn new() -> Self {
        MockDBClient {}
    }
}

impl BaseKVStorage for MockDBClient {
    fn put(&self, key: &str, value: &str) -> Result<()> {
        Ok(())
    }

    fn get(&self, key: &str) -> Option<String> {
        Some("Mock db response".to_string())
    }
}
