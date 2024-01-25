use tracing::{ info, debug };
use csv::{ ReaderBuilder, WriterBuilder };
use serde::Deserialize;
use std::error::Error;
use std::fs::{ File, OpenOptions };
use std::path::{ Path, PathBuf };
pub use crate::tasks::types::{ BaseTask, TaskRecord };
use tokio::time::{ sleep, Duration };

pub struct CacheService {
    pub task_id: String,
}

const CACHE_DIR_NAME: &str = "cache";
const DELIMITER: u8 = b',';

// cache to csv records
impl CacheService {
    pub fn new(task_id: String) -> Self {
        CacheService { task_id }
    }

    pub fn get_cache_file_path(&self) -> PathBuf {
        let current_dir = std::env::current_dir().unwrap();
        let cache_dir = current_dir.join(CACHE_DIR_NAME);
        std::fs::create_dir_all(&cache_dir).unwrap();
        let file_name = self.task_id.clone();
        let file_path = cache_dir.join(format!("{}.csv", file_name));

        file_path
    }

    pub async fn append_cache(&self, records: Vec<TaskRecord>) -> Result<(), Box<dyn Error>> {
        let file_path = self.get_cache_file_path();
        let file_exists = File::open(&file_path).is_ok();

        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true) // Create the file if it doesn't exist
            .open(&file_path).unwrap();

        let mut writer = WriterBuilder::new().has_headers(false).from_writer(file);

        for record in records {
            writer.serialize(record).unwrap();
        }

        writer.flush().unwrap();
        debug!("Cache has been appended to {:?}", file_path);

        Ok(())
    }
}
