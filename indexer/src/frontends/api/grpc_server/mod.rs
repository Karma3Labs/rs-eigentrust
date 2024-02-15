use crate::frontends::api::grpc_server::types::GRPCServerConfig;
use crate::tasks::cache::CacheService;
use crate::tasks::service::TaskService;
use proto_buf::indexer::indexer_server::{Indexer, IndexerServer};
use proto_buf::indexer::{IndexerEvent, Query};
use std::error::Error;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::info;

pub mod client;
pub mod types;

pub struct IndexerService {
	cache_file_path: PathBuf,
}

pub struct GRPCServer {
	config: GRPCServerConfig,
	task_service: TaskService,
}

impl IndexerService {
	fn new(cache_file_path: PathBuf) -> Self {
		IndexerService { cache_file_path }
	}
}

const CSV_COLUMN_INDEX_DATA: usize = 3;
const CSV_COLUMN_SCHEMA_ID: usize = 2;
const CSV_COLUMN_INDEX_TIMESTAMP: usize = 1;
const CSV_COLUMN_INDEX_ID: usize = 0;

#[tonic::async_trait]
impl Indexer for IndexerService {
	type SubscribeStream = ReceiverStream<Result<IndexerEvent, Status>>;

	async fn subscribe(
		&self, request: Request<Query>,
	) -> Result<Response<Self::SubscribeStream>, Status> {
		let inner = request.into_inner();

		let start = SystemTime::now();
		let _current_secs =
			start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

		let offset = inner.offset;
		let limit = inner.count;

		let cache_file_path = self.cache_file_path.clone().to_string_lossy().into_owned();

		let (tx, rx) = channel(128);
		tokio::spawn(async move {
			let records = CacheService::read(
				cache_file_path.into(),
				offset.try_into().unwrap(),
				limit.try_into().unwrap(),
			)
			.await;

			// todo also move to cache layer
			for (_index, record) in records.iter().enumerate() {
				let r = record.as_ref().unwrap();
				let event = IndexerEvent {
					id: r.get(CSV_COLUMN_INDEX_ID).unwrap().parse::<u32>().unwrap_or(0),
					schema_id: r.get(CSV_COLUMN_SCHEMA_ID).unwrap().parse::<u32>().unwrap_or(0),
					schema_value: r.get(CSV_COLUMN_INDEX_DATA).unwrap().to_string(),
					timestamp: r
						.get(CSV_COLUMN_INDEX_TIMESTAMP)
						.unwrap()
						.parse::<u64>()
						.unwrap_or(0),
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
		let address = format!("{}{}", "[::1]:", self.config.port.to_string()).parse()?;
		info!("GRPC server is starting at {}", address);

		// todo task id only
		let cache_file_path = self.task_service.cache.get_file_path();
		println!("{:?}", cache_file_path);
		let indexer_server = IndexerServer::new(IndexerService::new(cache_file_path));

		tokio::spawn(async move {
			let _ = Server::builder().add_service(indexer_server).serve(address).await;
		});

		// todo don't need to launch in server
		self.task_service.run().await;

		Ok(())
	}
}
