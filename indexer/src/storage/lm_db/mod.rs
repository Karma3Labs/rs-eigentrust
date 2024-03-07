use std::fs;

use heed::types::Str;
use heed::{Database, EnvOpenOptions};

use crate::storage::lm_db::types::LMDBClientConfig;
use crate::storage::types::KVStorageTrait;

pub mod types;

// todo change string to bytes?
#[derive(Clone)]
pub struct LMDBClient {
	db: Database<Str, Str>,
	env: heed::Env,
}

// https://github.com/meilisearch/heed/blob/main/heed/examples/all-types.rs
impl LMDBClient {
	pub fn new(config: LMDBClientConfig) -> Self {
		let _ = fs::create_dir_all(&config.path);

		let env = EnvOpenOptions::new()
			.map_size(config.map_size)
			.max_dbs(config.max_dbs)
			.open(&config.path)
			.unwrap();

		let db = env.create_database(Some(&config.db_name)).unwrap();
		LMDBClient { db, env }
	}
}

impl KVStorageTrait for LMDBClient {
	fn put(&self, key: &str, value: &str) -> heed::Result<()> {
		let mut write_txn = self.env.write_txn()?;
		self.db.put(&mut write_txn, key, value)?;
		write_txn.commit()?;
		Ok(())
	}

	fn get(&self, key: &str) -> Option<String> {
		let read_txn = self.env.read_txn().unwrap();
		let value = self.db.get(&read_txn, key).unwrap();
		value.map(|v| v.to_string())
	}
}
