use proto_buf;
use proto_buf::common::Void;
use proto_buf::transformer::transformer_client::TransformerClient;
use proto_buf::transformer::TermBatch;
use std::error::Error;
use tonic::transport::Channel;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let tr_channel = Channel::from_static("http://[::1]:50051").connect().await?;
	let mut tr_client = TransformerClient::new(tr_channel);

	// BasicRequest
	let void_request = Request::new(Void {});
	let response = tr_client.sync_indexer(void_request).await?.into_inner();
	println!("basic response {:?}", response);

	// BasicRequest
	let void_request = Request::new(TermBatch { start: 0, size: 1000 });
	let response = tr_client.term_stream(void_request).await?.into_inner();
	println!("basic response {:?}", response);

	Ok(())
}
