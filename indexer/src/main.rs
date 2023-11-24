use proto_buf::indexer::{
	indexer_server::{Indexer, IndexerServer},
	IndexerEvent, Query,
};
use std::{
	error::Error,
	time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

const FOLLOW_MOCK: &str = "{ 'id': '0x0', 'is_trustworthy': true, 'scope': 'Reviewer', 'sig': [ 0, [43, 43, 43, 54, 64, 67, 77, 87, 86, 67], [43, 43, 43, 54, 64, 67, 77, 87, 86, 67] ] }";

struct IndexerService;

#[tonic::async_trait]
impl Indexer for IndexerService {
	type SubscribeStream = ReceiverStream<Result<IndexerEvent, Status>>;
	async fn subscribe(
		&self, request: Request<Query>,
	) -> Result<Response<Self::SubscribeStream>, Status> {
		let inner = request.into_inner();

		let start = SystemTime::now();
		let current_secs = start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

		let (tx, rx) = channel(1);
		tokio::spawn(async move {
			for i in inner.offset..inner.offset + inner.count {
				let event = IndexerEvent {
					id: i,
					schema_id: 1,
					schema_value: FOLLOW_MOCK.to_string(),
					timestamp: current_secs,
				};
				tx.send(Ok(event)).await.unwrap();
			}
		});

		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = "[::1]:50050".parse()?;
	Server::builder().add_service(IndexerServer::new(IndexerService)).serve(addr).await?;
	Ok(())
}
