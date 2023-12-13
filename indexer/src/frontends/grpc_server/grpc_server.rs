use tracing::{ info, Level, debug };

use proto_buf::indexer::{ indexer_server::{ Indexer, IndexerServer }, IndexerEvent, Query };
use std::{ error::Error, time::{ SystemTime, UNIX_EPOCH } };
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{ transport::Server, Request, Response, Status };

use super::types::{ GRPCServerConfig };
use crate::tasks::service::{ TaskService };

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

pub struct IndexerService {}
pub struct GRPCServer {
    config: GRPCServerConfig,
    task_service: TaskService,
}

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

impl GRPCServer {
    pub fn new(config: GRPCServerConfig, task_service: TaskService) -> Self {
        GRPCServer { config, task_service }
    }

    pub async fn serve(&self) -> Result<(), Box<dyn Error>> {
        let address = format!("{}{}", "[::1]:", self.config.port.to_string()).parse()?;
        info!("GRPC server is starting at {}", address);
        Server::builder().add_service(IndexerServer::new(IndexerService {})).serve(address).await?;
        info!("GRPC server started at {}", address);
        Ok(())
    }
}
