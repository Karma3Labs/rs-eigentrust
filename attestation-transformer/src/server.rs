use proto::transformer_server::{Transformer, TransformerServer};
use proto::{RequestObject, ResponseObject};
use std::error::Error;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};
mod proto;

#[derive(Debug, Default)]
struct TransformerService;

#[tonic::async_trait]
impl Transformer for TransformerService {
    type ResponseStreamStream = ReceiverStream<Result<ResponseObject, Status>>;
    type BidirectionalStream = ReceiverStream<Result<ResponseObject, Status>>;

    async fn basic_request(
        &self,
        request: Request<RequestObject>,
    ) -> Result<Response<ResponseObject>, Status> {
        let req_obj = request.into_inner();
        println!("{:?}", req_obj.id);
        Ok(Response::new(ResponseObject {
            id: "basic_request".to_owned(),
        }))
    }

    async fn request_stream(
        &self,
        request: Request<Streaming<RequestObject>>,
    ) -> Result<Response<ResponseObject>, Status> {
        let mut stream = request.into_inner();
        let mut message = String::from("");
        while let Some(req) = stream.message().await? {
            message.push_str(&format!("Request {}\n", req.id));
        }
        println!("{:?}", message);
        Ok(Response::new(ResponseObject {
            id: "request_stream".to_owned(),
        }))
    }

    async fn response_stream(
        &self,
        request: Request<RequestObject>,
    ) -> Result<Response<Self::ResponseStreamStream>, Status> {
        let req_obj = request.into_inner();
        println!("{:?}", req_obj.id);
        let num_buffers = 4;
        let (tx, rx) = channel(num_buffers);
        tokio::spawn(async move {
            for _ in 0..num_buffers {
                tx.send(Ok(ResponseObject {
                    id: "response_stream".to_string(),
                }))
                .await
                .unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn bidirectional(
        &self,
        request: Request<Streaming<RequestObject>>,
    ) -> Result<Response<Self::BidirectionalStream>, Status> {
        let mut streamer = request.into_inner();
        let (tx, rx) = channel(4);
        tokio::spawn(async move {
            while let Some(req_obj) = streamer.message().await.unwrap() {
                println!("{:?}", req_obj.id);
                tx.send(Ok(ResponseObject {
                    id: format!("hello {}", req_obj.id),
                }))
                .await
                .unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "[::1]:50051".parse()?;
    let tr_service = TransformerService::default();

    Server::builder()
        .add_service(TransformerServer::new(tr_service))
        .serve(addr)
        .await?;

    Ok(())
}
