use proto_buf;
use proto_buf::transformer::transformer_client::TransformerClient;
use proto_buf::transformer::{EventBatch, TermBatch};
use std::error::Error;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tonic::Request;

const BATCH_SIZE: u32 = 1000;
const INTERVAL_SECS: u64 = 5;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let tr_channel = Channel::from_static("http://[::1]:50051").connect().await?;
	let mut tr_client = TransformerClient::new(tr_channel);

	let interval_size = Duration::from_secs(INTERVAL_SECS);
	let mut stream = IntervalStream::new(interval(interval_size));

	while let Some(_ts) = stream.next().await {
		let event_request = Request::new(EventBatch { size: BATCH_SIZE });
		let response = tr_client.sync_indexer(event_request).await?.into_inner();
		println!("sync_indexer response {:?}", response);

		if response.num_terms != 0 {
			let void_request = Request::new(TermBatch {
				start: response.total_count - response.num_terms,
				size: response.num_terms,
			});
			let response = tr_client.term_stream(void_request).await?.into_inner();
			println!("term_stream response {:?}", response);
		}
	}

	Ok(())
}
