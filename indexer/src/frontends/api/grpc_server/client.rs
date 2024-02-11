use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::{indexer_server::Indexer, Query};
use std::time::Duration;

use tonic::transport::Channel;
use tracing::{debug, info};

pub struct GRPCServerClient {}

const MAX_TERM_BATCH_SIZE: u32 = 1000;
const MAX_ATT_BATCH_SIZE: u32 = 1000;
const ATTESTATION_SOURCE_ADDRESS: &str = "0x1";
const AUDIT_APPROVE_SCHEMA_ID: &str = "0x2";
const AUDIT_DISAPPROVE_SCHEMA_ID: &str = "0x3";
const ENDORSE_SCHEMA_ID: &str = "0x4";

// for testing purpose
impl GRPCServerClient {
	pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
		// tokio::time::sleep(Duration::from_secs(1)).await;
		debug!("GRPC client is starting");

		let indexer_query = Query {
			source_address: ATTESTATION_SOURCE_ADDRESS.to_owned(),
			schema_id: vec![
				AUDIT_APPROVE_SCHEMA_ID.to_owned(),
				AUDIT_DISAPPROVE_SCHEMA_ID.to_owned(),
				ENDORSE_SCHEMA_ID.to_owned(),
			],
			offset: 0,
			count: 5,
		};

		let indexer_channel = Channel::from_static("http://[::1]:50050").connect().await.unwrap();

		info!("GRPC client started");
		let mut client = IndexerClient::new(indexer_channel.clone());
		let mut response = client.subscribe(indexer_query).await?.into_inner();
		let mut count = 0;
		while let Ok(Some(_res)) = response.message().await {
			// info!("{:?}", _res);
			count = count + 1;
		}
		info!("Got {:?} records", count);
		tokio::time::sleep(Duration::from_secs(1)).await;
		Ok(())
	}
}
