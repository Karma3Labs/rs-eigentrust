use tracing::info;

use proto_buf::indexer::{ indexer_server::{ Indexer, IndexerServer }, IndexerEvent, Query };
use std::{ error::Error, time::{ SystemTime, UNIX_EPOCH } };
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{ transport::Server, Request, Response, Status };

use super::types::GRPCServerConfig;
use crate::tasks::service::TaskService;
use crate::tasks::types::TaskResponse;
use std::cmp;
use std::sync::{ Arc, Mutex };
use flume::{ Sender, Receiver, bounded };

pub struct IndexerService {
    data: Vec<TaskResponse>,
    task_service_receiver: Receiver<i32>,
}
pub struct GRPCServer {
    config: GRPCServerConfig,
    task_service: TaskService,
}

impl IndexerService {
    fn new(data: Vec<TaskResponse>, task_service_receiver: Receiver<i32>) -> Self {
        println!("checking");
        for i in 0..10 {
            match task_service_receiver.recv() {
                Ok(msg) => println!("Received: {}", msg),
                Err(err) => println!("Error receiving: {}", err),
            }
        }

        IndexerService { data, task_service_receiver }
    }
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
        let limit = cmp::min(inner.offset + inner.count, self.data.len().try_into().unwrap());

        let data = self.data.clone();

        match self.task_service_receiver.recv() {
            Ok(msg) => println!("Received: {}", msg),
            Err(err) => println!("Error receiving: {}", err),
        }

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
        let address = format!("{}{}", "[::1]:", self.config.port.to_string()).parse()?;
        info!("GRPC server is starting at {}", address);
        self.task_service.run().await;

        // todo
        let data = self.task_service.get_chunk(0, 10000).await;

        // Create a bounded channel with a capacity of 5
        let (task_service_publisher, task_service_receiver): (Sender<i32>, Receiver<i32>) = bounded(
            5
        );

        // Spawn a thread to send messages
        let sender_thread = std::thread::spawn(move || {
            for i in 0..10 {
                match task_service_publisher.send(i) {
                    Ok(_) => println!("Sent: {}", i),
                    Err(err) => println!("Error sending: {}", err),
                }

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        // Wait for threads to finish

        // sender_thread.join().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3000));

        let indexer_server = IndexerServer::new(IndexerService::new(data, task_service_receiver));
        Server::builder().add_service(indexer_server).serve(address).await?;

        Ok(())
    }
}
