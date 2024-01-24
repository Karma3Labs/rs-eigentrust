use eyre::Result;
use std::error::Error;
use std::fs::File;
use tracing::{ debug, info };
use reqwest;
use reqwest::{ Response };
use serde_json::{ Value, from_str };

use super::types::MetamaskConnectorClientConfig;
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
        &self,
        from: Option<u64>,
        range: Option<u64>
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let offset = from.unwrap_or(0);
        let limit = range.unwrap_or(DEFAULT_LIMIT);
        let url = &self.config.url.clone();

        let response = reqwest::get(url).await?.json::<Vec<Value>>().await?;
        let records: Vec<String> = response
            .iter()
            .map(|value| value.to_string())
            .collect();

        Ok(records)
    }
}
