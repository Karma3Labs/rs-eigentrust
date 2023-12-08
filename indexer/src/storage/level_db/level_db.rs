use leveldb::database::Database;
use leveldb::kv::KV;
use leveldb::options::{ Options, WriteOptions, ReadOptions };
use serde::{ Serialize, Deserialize };
use std::path::Path;

pub struct LevelDBClient {
    db: Database<i32>,
}

// todo implement generic interface
impl LevelDBClient {
    pub fn new(db_path: &str) -> Self {
        let path = Path::new(db_path);
        let mut options = Options::new();
        options.create_if_missing = true;

        let db = Database::open(path, options).unwrap();

        LevelDBClient { db }
    }

    /*
    pub fn put(&self, key_str: &str, value_str: &str) -> Result<(), leveldb::error::Error> {
        
    }

    pub fn get(&self, key_str: &str) -> Option<String> {
        
    }
     */
}
