use error::AttTrError;
use proto_buf::common::Void;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::{IndexerEvent, Query};
use proto_buf::transformer::transformer_server::{Transformer, TransformerServer};
use proto_buf::transformer::{TermBatch, TermObject};
use rocksdb::{WriteBatch, DB};
use schemas::{AuditApproveSchema, AuditDisapproveSchema, FollowSchema, SchemaType};
use serde_json::from_str;
use std::error::Error;
use term::{IntoTerm, Term};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

mod did;
mod error;
mod schemas;
mod term;
mod utils;

const MAX_TERM_BATCH_SIZE: u32 = 1000;
const MAX_ATT_BATCH_SIZE: u32 = 1000;
const ATTESTATION_SOURCE_ADDRESS: &str = "0x1";
const FOLLOW_SCHEMA_ID: &str = "0x2";

#[derive(Debug)]
struct TransformerService {
	channel: Channel,
	db: String,
}

impl TransformerService {
	fn new(channel: Channel, db_url: &str) -> Result<Self, AttTrError> {
		let db = DB::open_default(db_url).map_err(|x| AttTrError::DbError(x))?;
		let checkpoint = db.get(b"checkpoint").map_err(|x| AttTrError::DbError(x))?;
		if let None = checkpoint {
			let count = 0u32.to_be_bytes();
			db.put(b"checkpoint", count).map_err(|x| AttTrError::DbError(x))?;
		}

		Ok(Self { channel, db: db_url.to_string() })
	}

	fn read_checkpoint(db: &DB) -> Result<u32, AttTrError> {
		let offset_bytes_opt = db.get(b"checkpoint").map_err(|x| AttTrError::DbError(x))?;
		let offset_bytes = offset_bytes_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		let offset = u32::from_be_bytes(offset_bytes);
		Ok(offset)
	}

	fn write_checkpoint(db: &DB, count: u32) -> Result<(), AttTrError> {
		db.put(b"checkpoint", count.to_be_bytes()).map_err(|x| AttTrError::DbError(x))?;
		Ok(())
	}

	fn read_terms(db: &DB, batch: TermBatch) -> Result<Vec<TermObject>, AttTrError> {
		let mut terms = Vec::new();
		for i in batch.start..batch.size {
			let id_bytes = i.to_be_bytes();
			let res_opt = db.get(id_bytes).map_err(|x| AttTrError::DbError(x))?;
			let res = res_opt.ok_or_else(|| AttTrError::NotFoundError)?;
			let term = Term::from_bytes(res)?;
			let term_obj: TermObject = term.into();
			terms.push(term_obj);
		}
		Ok(terms)
	}

	fn parse_event(event: IndexerEvent) -> Result<(u32, Term), AttTrError> {
		let schema_id = event.schema_id;
		let schema_type = SchemaType::from(schema_id);
		let term = match schema_type {
			SchemaType::Follow => {
				let parsed_att: FollowSchema =
					from_str(&event.schema_value).map_err(|_| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
			SchemaType::AuditApprove => {
				let parsed_att: AuditApproveSchema =
					from_str(&event.schema_value).map_err(|_| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
			SchemaType::AuditDisapprove => {
				let parsed_att: AuditDisapproveSchema =
					from_str(&event.schema_value).map_err(|_| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
		};

		Ok((event.id, term))
	}

	fn write_terms(db: &DB, terms: Vec<(u32, Term)>) -> Result<(), AttTrError> {
		let mut batch = WriteBatch::default();
		for (id, term) in terms {
			let term_bytes = term.into_bytes()?;
			let id = id.to_be_bytes();
			batch.put(id, term_bytes);
		}
		db.write(batch).map_err(|e| AttTrError::DbError(e))
	}
}

#[tonic::async_trait]
impl Transformer for TransformerService {
	type TermStreamStream = ReceiverStream<Result<TermObject, Status>>;

	async fn sync_indexer(&self, _: Request<Void>) -> Result<Response<Void>, Status> {
		let db = DB::open_default(self.db.clone())
			.map_err(|_| Status::internal("Failed to connect to DB"))?;

		let offset = Self::read_checkpoint(&db)
			.map_err(|_| Status::internal("Failed to read checkpoint"))?;

		let indexer_query = Query {
			source_address: ATTESTATION_SOURCE_ADDRESS.to_owned(),
			schema_id: vec![FOLLOW_SCHEMA_ID.to_owned()],
			offset,
			count: MAX_ATT_BATCH_SIZE,
		};

		let mut client = IndexerClient::new(self.channel.clone());
		let mut response = client.subscribe(indexer_query).await?.into_inner();
		let mut count = offset;
		let mut terms = Vec::new();
		// ResponseStream
		while let Ok(Some(res)) = response.message().await {
			assert!(res.id == count);
			let term =
				Self::parse_event(res).map_err(|_| Status::internal("Failed to parse event"))?;
			terms.push(term);
			count += 1;
		}

		Self::write_terms(&db, terms).map_err(|_| Status::internal("Failed to write terms"))?;
		Self::write_checkpoint(&db, count)
			.map_err(|_| Status::internal("Failed to write checkpoint"))?;

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

		let db = DB::open_default(self.db.clone())
			.map_err(|_| Status::internal("Failed to connect to DB"))?;

		let terms =
			Self::read_terms(&db, inner).map_err(|_| Status::internal("Failed to read terms"))?;

		let (tx, rx) = channel(1);
		for term in terms {
			let res = tx.send(Ok(term)).await.map_err(|x| x.0);
			if let Err(err) = res {
				err?;
			};
		}

		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let channel = Channel::from_static("http://localhost:50050").connect().await?;
	let db_url = "att-tr-storage";
	let tr_service = TransformerService::new(channel, db_url)?;

	let addr = "[::1]:50051".parse()?;
	Server::builder().add_service(TransformerServer::new(tr_service)).serve(addr).await?;
	Ok(())
}

#[cfg(test)]
mod test {
	use proto_buf::indexer::IndexerEvent;
	use proto_buf::transformer::{TermBatch, TermObject};
	use rocksdb::DB;
	use serde_json::to_string;

	use crate::schemas::Scope;
	use crate::term::IntoTerm;
	use crate::{schemas::FollowSchema, TransformerService};

	#[test]
	fn should_write_read_checkpoint() {
		let db = DB::open_default("att-tr-checkpoint-test-storage").unwrap();
		TransformerService::write_checkpoint(&db, 15).unwrap();
		let checkpoint = TransformerService::read_checkpoint(&db).unwrap();
		assert_eq!(checkpoint, 15);
	}

	#[test]
	fn should_write_read_term() {
		let db = DB::open_default("att-tr-terms-test-storage").unwrap();

		let follow_schema = FollowSchema::new(
			"did:pkh:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned(),
			true,
			Scope::Auditor,
		);
		let indexed_event = IndexerEvent {
			id: 0,
			schema_id: 1,
			schema_value: to_string(&follow_schema).unwrap(),
			timestamp: 2397848,
		};
		let term = TransformerService::parse_event(indexed_event).unwrap();
		TransformerService::write_terms(&db, vec![term]).unwrap();

		let term_batch = TermBatch { start: 0, size: 1 };
		let terms = TransformerService::read_terms(&db, term_batch).unwrap();

		let term = follow_schema.into_term().unwrap();
		let term_obj: TermObject = term.into();
		assert_eq!(terms, vec![term_obj]);
	}
}
