use proto_buf;
use proto_buf::transformer::transformer_client::TransformerClient;
use proto_buf::transformer::Void;
use std::error::Error;
use tonic::transport::Channel;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let channel = Channel::from_static("http://[::1]:50051").connect().await?;
	let mut client = TransformerClient::new(channel);

	// BasicRequest
	let request = Request::new(Void {});
	let response = client.sync_indexer(request).await?.into_inner();
	println!("basic response {:?}", response);

	Ok(())
}
