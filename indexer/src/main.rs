mod config;
mod logger;
mod clients;
mod tasks;
mod storage;

use tracing::{ info, Level };

use proto_buf::indexer::{ indexer_server::{ Indexer, IndexerServer }, IndexerEvent, Query };
use std::{ error::Error, time::{ SystemTime, UNIX_EPOCH } };
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{ transport::Server, Request, Response, Status };

use crate::tasks::service::TaskService;
use crate::storage::level_db::level_db::LevelDBClient;

// todo clean this up
const FOLLOW_MOCK: &str =
    "{
    \"id\": \"0x0\",
    \"is_trustworthy\": true,
    \"scope\": \"Reviewer\",
    \"sig\": [
        0,
        [165, 27, 231, 102, 0, 210, 165, 235, 176, 250, 84, 181, 240, 246, 182, 135, 85, 181, 106, 145, 41, 107, 207, 81, 49, 37, 133, 183, 171, 151, 67, 67],
        [116, 33, 248, 224, 110, 187, 80, 139, 81, 22, 199, 37, 68, 255, 180, 55, 159, 59, 232, 70, 206, 232, 38, 165, 54, 233, 19, 31, 57, 139, 186, 54]
    ]
}";

struct IndexerService;

#[tonic::async_trait]
impl Indexer for IndexerService {
    type SubscribeStream = ReceiverStream<Result<IndexerEvent, Status>>;
    async fn subscribe(
        &self,
        request: Request<Query>
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
async fn main() {
    //-> Result<(), Box<dyn Error>> {
    let config = config::Config::from_env();
    let logger_config = config.logger_config.clone();

    let logger: logger::factory::AppLogger = logger::factory::AppLogger::new(logger_config);
    logger.init_global_default();

    let client_config = config.evm_indexer_config.clone();
    let client = clients::clique::client::CliqueClient::new(client_config);

    let clique_task_config = config.evm_indexer_config.clone();
    let clique_task = tasks::clique::task::CliqueTask::new(clique_task_config, client);

    let db_path = concat!(env!("CARGO_MANIFEST_DIR"), "/db");
    let db = LevelDBClient::new(db_path);
    /*
    db.put("hello", "hello");
    let r = db.get("hello").unwrap_or("not found".to_string());
    println!("{}", r);
     */

    let mut task_service = TaskService::new(Box::new(clique_task));
    task_service.run().await;

    // let addr = "[::1]:50050".parse()?;
    // Server::builder().add_service(IndexerServer::new(IndexerService)).serve(addr).await?;
    // Ok(())
}
