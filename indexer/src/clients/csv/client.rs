use std::error::Error;
use std::fs::File;

use csv::{ReaderBuilder, StringRecord};
use eyre::Result;
use tracing::debug;

use crate::clients::csv::types::CSVClientConfig;

pub struct CSVClient {
	config: CSVClientConfig,
}

const DEFAULT_OFFSET: u64 = 1024;

impl CSVClient {
	pub fn new(config: CSVClientConfig) -> Self {
		debug!("CSV client created");
		CSVClient { config }
	}

	pub async fn query(
		&self, from: Option<u64>, range: Option<u64>,
	) -> Result<Vec<Result<StringRecord, csv::Error>>, Box<dyn Error>> {
		let file = File::open(&self.config.path)?;

		let offset = from.unwrap_or(0);
		let limit = range.unwrap_or(DEFAULT_OFFSET);

		// todo no header
		let mut csv_reader = ReaderBuilder::new().delimiter(b';').from_reader(file);

		// todo ??? skip records
		for _ in 0..offset {
			csv_reader.records().next();
		}

		let records: Vec<Result<StringRecord, csv::Error>> =
			csv_reader.into_records().take(limit.try_into().unwrap()).collect();

		Ok(records)
	}
}
