use serde::Deserialize;
use serde_json::Value;
#[derive(Clone, Debug)]
pub struct MetamaskConnectorClientConfig {
	pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetamaskAPIRecord {
	pub id: usize,
	pub assertion: Value,
	pub creation_at: String,
}
