use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::Query;
use proto_buf::transformer::transformer_server::{Transformer, TransformerServer};
use proto_buf::transformer::{TermBatch, TermObject, Void};
use rocksdb::DB;
use schemas::FollowSchema;
use serde_json::from_str;
use std::error::Error;
use term::{IntoTerm, Term};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

mod schemas;
mod term;

const MAX_TERM_BATCH_SIZE: u32 = 1000;

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
	type TermStreamStream = ReceiverStream<Result<TermObject, Status>>;

	async fn sync_indexer(&self, request: Request<Query>) -> Result<Response<Void>, Status> {
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
				let parsed_att: FollowSchema = from_str(&res.schema_value).unwrap();
				let term: Term = parsed_att.into_term();
				let term_bytes = term.into_bytes();
				db.put(id, &term_bytes).unwrap();

				db.put(b"checkpoint", count.to_be_bytes()).unwrap();
			}
		});

		Ok(Response::new(Void::default()))
	}

	async fn term_stream(
		&self, request: Request<TermBatch>,
	) -> Result<Response<Self::TermStreamStream>, Status> {
		let inner = request.into_inner();
		if inner.size > MAX_TERM_BATCH_SIZE {
			return Result::Err(Status::invalid_argument(format!(
				"Batch size too big. Max size: {}",
				MAX_TERM_BATCH_SIZE
			)));
		}

		let mut terms = Vec::new();
		let db = DB::open_default(self.db.clone()).unwrap();
		for i in inner.start..inner.size {
			let id_bytes = i.to_be_bytes();
			let res = db.get(id_bytes).unwrap().unwrap();
			let term = Term::from_bytes(res);
			let term_obj: TermObject = term.into();
			terms.push(term_obj);
		}

		let (tx, rx) = channel(1);
		tokio::spawn(async move {
			for term in terms {
				tx.send(Ok(term)).await.unwrap();
			}
		});

		Ok(Response::new(ReceiverStream::new(rx)))
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
