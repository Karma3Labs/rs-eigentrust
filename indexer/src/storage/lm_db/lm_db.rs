use heed::{ EnvOpenOptions, Database, RoTxn, RwTxn, Result };
use std::path::Path;
use heed::types::*;
use super::types::{ LMDBClientConfig };
use std::fs;
use crate::storage::types::{ BaseKVStorage };

pub struct LMDBClient {
    db: Database<Str, Str>,
    env: heed::Env,
}

// todo change string to bytes
// https://github.com/meilisearch/heed/blob/main/heed/examples/all-types.rs
impl LMDBClient {
    pub fn new(config: LMDBClientConfig) -> Self {
        fs::create_dir_all(&config.path);

        let env = EnvOpenOptions::new()
            .map_size(10 * 1024 * 1024) // 10 mb
            .max_dbs(3000)
            .open(&config.path)
            .unwrap();

        let db = env.create_database(Some(&config.db_name)).unwrap();
        LMDBClient { db, env }
    }
}

impl BaseKVStorage for LMDBClient {
    fn put(&self, key: &str, value: &str) -> Result<()> {
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
