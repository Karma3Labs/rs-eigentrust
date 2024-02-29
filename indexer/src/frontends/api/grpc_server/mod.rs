use std::cmp;
use std::error::Error;
use std::time::SystemTime;

use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::info;

use proto_buf::indexer::indexer_server::{Indexer, IndexerServer};
use proto_buf::indexer::{IndexerEvent, Query};

use crate::frontends::api::grpc_server::types::GRPCServerConfig;
use crate::tasks::service::TaskService;
use crate::tasks::types::TaskResponse;

pub mod types;

pub struct IndexerService {
	data: Vec<TaskResponse>,
}

pub struct GRPCServer {
	config: GRPCServerConfig,
	task_service: TaskService,
}

impl IndexerService {
	fn new(data: Vec<TaskResponse>) -> Self {
		IndexerService { data }
	}
}

#[tonic::async_trait]
impl Indexer for IndexerService {
	type SubscribeStream = ReceiverStream<Result<IndexerEvent, Status>>;

	async fn subscribe(
		&self, request: Request<Query>,
	) -> Result<Response<Self::SubscribeStream>, Status> {
		let inner = request.into_inner();

		let _start = SystemTime::now();
		// let current_secs = _start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();
		let limit = cmp::min(
			inner.offset + inner.count,
			self.data.len().try_into().unwrap(),
		);

		let data = self.data.clone();

		let (tx, rx) = channel(4);
		tokio::spawn(async move {
			for i in inner.offset..limit {
				let index: usize = i as usize;

				let record = data[index].clone();

				let event = IndexerEvent {
					id: i + 1,
					schema_id: record.schema_id as u32,
					schema_value: record.data,
					timestamp: record.timestamp.parse::<u64>().unwrap(),
				};
				tx.send(Ok(event)).await.unwrap();
			}
		});

		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

impl GRPCServer {
	pub fn new(config: GRPCServerConfig, task_service: TaskService) -> Self {
		GRPCServer { config, task_service }
	}

	pub async fn serve(&mut self) -> Result<(), Box<dyn Error>> {
		let address = format!("{}{}", "[::1]:", self.config.port).parse()?;
		info!("GRPC server is starting at {}", address);
		self.task_service.run().await;

		// todo
		let data = self.task_service.get_chunk(0, 10000).await;

		let indexer_server = IndexerServer::new(IndexerService::new(data));
		Server::builder().add_service(indexer_server).serve(address).await?;

		Ok(())
	}
}
