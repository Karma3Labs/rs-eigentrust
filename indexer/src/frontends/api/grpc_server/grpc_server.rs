use csv::{ ReaderBuilder, StringRecord };
use proto_buf::indexer::{ indexer_server::{ Indexer, IndexerServer }, IndexerEvent, Query };
use std::fs::File;
use std::path::PathBuf;
use std::{ error::Error, time::{ SystemTime, UNIX_EPOCH } };
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{ transport::Server, Request, Response, Status };
use tracing::info;

use super::types::GRPCServerConfig;
use crate::tasks::cache::CacheService;
use crate::tasks::service::TaskService;
use crate::tasks::types::TaskRecord;

use std::cmp;

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

const DELIMITER: u8 = b',';
const CSV_COLUMN_INDEX_DATA: usize = 3;
const CSV_COLUMN_SCHEMA_ID: usize = 2;
const CSV_COLUMN_INDEX_TIMESTAMP: usize = 1;

#[tonic::async_trait]
impl Indexer for IndexerService {
    type SubscribeStream = ReceiverStream<Result<IndexerEvent, Status>>;

    async fn subscribe(
        &self,
        request: Request<Query>
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        println!("grpc requested");

        let inner = request.into_inner();

        let start = SystemTime::now();
        let _current_secs = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let offset = inner.offset;
        let limit = inner.offset + inner.count;

        let cache_file_path = self.cache_file_path.clone().to_string_lossy().into_owned();

        let (tx, rx) = channel(128);
        tokio::spawn(async move {
            let records = CacheService::read(
                cache_file_path.into(),
                offset.try_into().unwrap(),
                limit.try_into().unwrap()
            ).await;

            for (index, record) in records.iter().enumerate() {
                let r = record.as_ref().unwrap();

                let event = IndexerEvent {
                    id: (index as u32) + (offset as u32),
                    schema_id: r.get(CSV_COLUMN_SCHEMA_ID).unwrap().parse::<u32>().unwrap_or(0),
                    schema_value: r.get(CSV_COLUMN_INDEX_DATA).unwrap().to_string(),
                    timestamp: r
                        .get(CSV_COLUMN_INDEX_TIMESTAMP)
                        .unwrap()
                        .parse::<u64>()
                        .unwrap_or(0),
                };

                println!("{:?}", event);
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

        // todo ??
        let cache_file_path = self.task_service.cache.get_cache_file_path();
        let indexer_server = IndexerServer::new(IndexerService::new(cache_file_path));

        tokio::spawn(async move {
            Server::builder().add_service(indexer_server).serve(address).await;
        });

        self.task_service.run().await;

        Ok(())
    }
}
