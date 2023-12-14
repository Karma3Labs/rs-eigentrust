use heed::{ EnvOpenOptions, Database, RoTxn, RwTxn, Result };
use std::path::Path;
use heed::types::*;
use super::types::{ LMDBClientConfig };
use std::fs;

pub struct LMDBClient {
    db: Database<Str, Str>,
    env: heed::Env,
}

// change string to bytes
// https://github.com/meilisearch/heed/blob/main/heed/examples/all-types.rs
impl LMDBClient {
    pub fn new(db_path: &str) -> Self {
        fs::create_dir_all(&db_path);

        let env = EnvOpenOptions::new()
            .map_size(10 * 1024 * 1024) // 10 mb
            .max_dbs(3000)
            .open(db_path)
            .unwrap();

        let db = env.create_database(Some("key_value_storage")).unwrap();
        LMDBClient { db, env }
    }

    pub fn put(&self, key: &str, value: &str) -> Result<()> {
        let mut write_txn = self.env.write_txn()?;
        self.db.put(&mut write_txn, key, value)?;
        write_txn.commit()?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let read_txn = self.env.read_txn()?;
        let value = self.db.get(&read_txn, key)?;
        Ok(value.map(|v| v.to_string()))
    }
}
