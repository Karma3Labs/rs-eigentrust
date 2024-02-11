use eyre::Result;
use reqwest;


use std::error::Error;

use tracing::debug;

use super::types::{MetamaskAPIRecord, MetamaskConnectorClientConfig};
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
		let urlPath = format!("{}/assertions/?from={}&to={}", url, _offset, _limit);

		let records = reqwest::get(urlPath).await?.json::<Vec<MetamaskAPIRecord>>().await?;
		Ok(records)
	}
}
