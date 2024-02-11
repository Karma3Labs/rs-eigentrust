use serde::Deserialize;
use serde_json::Value;
#[derive(Clone, Debug)]
pub struct MetamaskConnectorClientConfig {
	pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct MetamaskAPIRecord {
	pub id: usize,
	pub assertion: Value,
	pub creationAt: String,
}
