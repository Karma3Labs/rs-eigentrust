use futures::stream::iter;
use proto::transformer_client::TransformerClient;
use std::error::Error;
use tonic::transport::Channel;

use proto::RequestObject;
use tonic::Request;

mod proto;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let channel = Channel::from_static("http://[::1]:50051").connect().await?;
	let mut client = TransformerClient::new(channel);

	// BasicRequest
	let request = Request::new(RequestObject { id: "0".to_string(), scope: 0 });
	let response = client.basic_request(request).await?.into_inner();
	println!("basic response {:?}", response);

	// RequestStream
	let request = Request::new(iter(vec![
		RequestObject { id: "1".to_string(), scope: 0 },
		RequestObject { id: "2".to_string(), scope: 0 },
	]));
	let response = client.request_stream(request).await?.into_inner();
	println!("response to stream request {:?}", response);

	// ResponseStream
	let request = Request::new(RequestObject { id: "3".to_string(), scope: 0 });
	let mut response = client.response_stream(request).await?.into_inner();
	while let Some(res) = response.message().await? {
		println!("response stream {:?}", res);
	}

	// Bidirectional
	let request = Request::new(iter(vec![RequestObject { id: "4".to_string(), scope: 0 }]));
	let mut response = client.bidirectional(request).await?.into_inner();
	while let Some(res) = response.message().await? {
		println!("bidirectional {:?}", res);
	}

	Ok(())
}
