use eyre::Result;
use reqwest;
use std::error::Error;
use std::time::Duration;
use tracing::{debug, info};

use super::types::{
	MetamaskAPIRecord, MetamaskConnectorClientConfig, MetamaskGetAssertionsResponse,
};
pub use crate::clients::types::EVMLogsClient;

pub struct MetamaskConnectorClient {
	pub config: MetamaskConnectorClientConfig,
}

const DEFAULT_LIMIT: u64 = 1024;

impl MetamaskConnectorClient {
	pub fn new(config: MetamaskConnectorClientConfig) -> Self {
		debug!("Metamask connector client created");
		MetamaskConnectorClient { config }
	}

	pub async fn query(
		&self, from: Option<u64>, range: Option<u64>,
	) -> Result<Vec<MetamaskAPIRecord>, Box<dyn Error>> {
		let from_unwrapped = from.unwrap_or(0);
		let _offset = from_unwrapped + 1; // starts from 1 not 0

		let _limit = range.unwrap_or(DEFAULT_LIMIT);
		let url = &self.config.url;
		let url_path = format!("{}/assertions/?from={}&to={}", url, _offset, _limit);

		let mut delay = Duration::from_secs(0);
		let records = loop {
			let err = match reqwest::get(&url_path).await {
				Ok(response) => match response.error_for_status() {
					Ok(response) => match response.json::<MetamaskGetAssertionsResponse>().await {
						Ok(records) => break records,
						Err(err) => err,
					},
					Err(err) => err,
				},
				Err(err) => err,
			};
			delay += Duration::from_secs(1);
			delay *= 2;
			let max_delay = Duration::from_secs(30);
			if delay > max_delay {
				delay = max_delay;
			}
			info!(?err, ?delay, "assertion polling failed, retrying");
		};
		Ok(records.assertions)
	}
}
