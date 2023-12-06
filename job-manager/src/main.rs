use proto_buf;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::common::Void;
use proto_buf::transformer::transformer_client::TransformerClient;
use std::error::Error;
use tonic::transport::Channel;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let tr_channel = Channel::from_static("http://[::1]:50051").connect().await?;
	let mut tr_client = TransformerClient::new(tr_channel);

	let lc_channel = Channel::from_static("http://[::1]:50052").connect().await?;
	let mut lc_channel = LinearCombinerClient::new(lc_channel);

	// BasicRequest
	let void_request = Request::new(Void {});
	let response = tr_client.sync_indexer(void_request).await?.into_inner();
	println!("basic response {:?}", response);

	// BasicRequest
	// let void_request = Request::new(Void {});
	// let response = lc_channel.sync_transformer(void_request).await?.into_inner();
	// println!("basic response {:?}", response);

	Ok(())
}
