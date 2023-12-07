use leveldb::database::Database;
use leveldb::kv::KV;
use leveldb::options::{ Options, WriteOptions, ReadOptions };
use serde::{ Serialize, Deserialize };

struct LevelDBClient {
    db: Database<String>,
}

impl LevelDBClient {
    pub fn new(db_path: &str) -> Result<Self, leveldb::error::Error> {
        let mut options = Options::new();
        options.create_if_missing = true;
        let db = Database::open(db_path, options)?;

        Ok(Self { db })
    }

    pub fn save<T>(&self, key: &str, value: &T) -> Result<(), leveldb::error::Error>
        where T: Serialize
    {
        let serialized_value = serde_json::to_string(value)?;
        let write_options = WriteOptions::new();
        self.db.put(write_options, key, serialized_value)?;

        Ok(())
    }

    pub fn load<T>(&self, key: &str) -> Result<Option<T>, leveldb::error::Error>
        where T: Deserialize<'static>
    {
        let read_options = ReadOptions::new();
        if let Some(serialized_value) = self.db.get(read_options, key)? {
            let deserialized_value: T = serde_json::from_str(&serialized_value)?;
            Ok(Some(deserialized_value))
        } else {
            Ok(None)
        }
    }
}
