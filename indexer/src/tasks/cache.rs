pub use crate::tasks::types::TaskRecord;
use csv::{ReaderBuilder, StringRecord, WriterBuilder};

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use tracing::debug;

pub struct CacheService {
	pub task_id: String,
}

const CACHE_DIR_NAME: &str = "cache";
const DELIMITER: u8 = b',';

// todo trait for testing purposes
// cache to csv records
impl CacheService {
	pub fn new(task_id: String) -> Self {
		CacheService { task_id }
	}

	pub fn get_file_path(&self) -> PathBuf {
		let current_dir = std::env::current_dir().unwrap();
		let cache_dir = current_dir.join(CACHE_DIR_NAME);
		std::fs::create_dir_all(&cache_dir).unwrap();
		let file_name = self.task_id.clone();
		let file_path = cache_dir.join(format!("{}.csv", file_name));

		file_path
	}

	// todo gaps in syncing?
	pub async fn append(&self, records: Vec<TaskRecord>) -> Result<(), Box<dyn Error>> {
		let file_path = self.get_file_path();
		let _file_exists = File::open(&file_path).is_ok();

		let file = OpenOptions::new()
			.write(true)
			.append(true)
			.create(true) // Create the file if it doesn't exist
			.open(&file_path)
			.unwrap();

		let mut writer =
			WriterBuilder::new().delimiter(DELIMITER).has_headers(false).from_writer(file);

		for record in records {
			writer.serialize(record).unwrap();
		}

		writer.flush().unwrap();
		debug!("Cache has been appended to {:?}", file_path);

		Ok(())
	}

	// perfomance of accessing the data. read from end of the file?
	// filenames with postfix 0_1000_000 etc
	pub async fn read(
		file_path: PathBuf, offset: usize, limit: usize,
	) -> Vec<Result<StringRecord, csv::Error>> {
		let file: File = File::open(file_path).unwrap();

		let mut csv_reader =
			ReaderBuilder::new().has_headers(false).delimiter(DELIMITER).from_reader(file);

		for _ in 0..offset {
			if csv_reader.records().next().is_none() {
				// Break if there are fewer than N records in the file
				break;
			}
		}

		//for _i in 0..limit {
		//    csv_reader.records().next();
		//}

		let records: Vec<Result<StringRecord, csv::Error>> =
			csv_reader.into_records().take(limit).collect();

		records
	}
}
