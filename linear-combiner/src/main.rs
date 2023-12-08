use proto_buf::{
	combiner::{
		linear_combiner_server::{LinearCombiner, LinearCombinerServer},
		LtBatch, LtObject,
	},
	common::Void,
	transformer::TermObject,
};
use std::error::Error;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

struct LinearCombinerService;

#[tonic::async_trait]
impl LinearCombiner for LinearCombinerService {
	type SyncCoreComputerStream = ReceiverStream<Result<LtObject, Status>>;

	async fn sync_transformer(
		&self, request: Request<Streaming<TermObject>>,
	) -> Result<Response<Void>, Status> {
		let mut stream = request.into_inner();
		while let Some(req) = stream.message().await? {
			println!("{:?}", req.from);
		}
		Ok(Response::new(Void {}))
	}

	async fn sync_core_computer(
		&self, request: Request<LtBatch>,
	) -> Result<Response<Self::SyncCoreComputerStream>, Status> {
		let req_obj = request.into_inner();
		let num_buffers = 4;
		let (tx, rx) = channel(num_buffers);
		for _ in 0..num_buffers {
			tx.send(Ok(LtObject { x: 0, y: 0, value: 0 })).await.unwrap();
		}
		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = "[::1]:50052".parse()?;
	Server::builder()
		.add_service(LinearCombinerServer::new(LinearCombinerService))
		.serve(addr)
		.await?;
	Ok(())
}
