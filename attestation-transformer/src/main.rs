use prost::Message;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::QueryVerax;
use proto_buf::transformer::transformer_server::{Transformer, TransformerServer};
use proto_buf::transformer::Void;
use rocksdb::DB;
use std::error::Error;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug)]
struct TransformerService {
	channel: Channel,
	db: String,
}

impl TransformerService {
	fn new(channel: Channel, db_url: &str) -> Self {
		let db = DB::open_default(db_url).unwrap();
		let checkpoint = db.get(b"checkpoint").unwrap();
		if let None = checkpoint {
			let count = 0u32.to_be_bytes();
			db.put(b"checkpoint", count).unwrap();
		}

		Self { channel, db: db_url.to_string() }
	}
}

#[tonic::async_trait]
impl Transformer for TransformerService {
	async fn sync_verax(&self, request: Request<QueryVerax>) -> Result<Response<Void>, Status> {
		let req_obj = request.into_inner();
		let request = Request::new(req_obj);
		let mut client = IndexerClient::new(self.channel.clone());
		let mut response = client.subscribe(request).await?.into_inner();

		let db_url = self.db.clone();
		tokio::spawn(async move {
			// ResponseStream
			let db = DB::open_default(db_url).unwrap();

			let mut bytes: [u8; 4] = [0; 4];
			let count_bytes = db.get(b"checkpoint").unwrap().unwrap();
			bytes.copy_from_slice(&count_bytes);

			let mut count = u32::from_be_bytes(bytes);
			while let Some(res) = response.message().await.unwrap() {
				count += 1;
				assert!(res.id == count);

				let id = res.id.to_be_bytes();
				let data = res.encode_to_vec();
				db.put(id, &data).unwrap();

				db.put(b"checkpoint", count.to_be_bytes()).unwrap();
			}
		});

		Ok(Response::new(Void::default()))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let channel = Channel::from_static("[::1]:50052").connect().await?;
	let db_url = "att-tr-storage";
	let tr_service = TransformerService::new(channel, db_url);

	let addr = "[::1]:50051".parse()?;
	Server::builder().add_service(TransformerServer::new(tr_service)).serve(addr).await?;
	Ok(())
}
