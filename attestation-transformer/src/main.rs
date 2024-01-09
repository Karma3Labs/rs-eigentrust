use error::AttTrError;
use futures::stream::iter;
use itertools::Itertools;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::common::Void;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::{IndexerEvent, Query};
use proto_buf::transformer::transformer_server::{Transformer, TransformerServer};
use proto_buf::transformer::{TermBatch, TermObject};
use rocksdb::{WriteBatch, DB};
use schemas::status::StatusSchema;
use schemas::trust::TrustSchema;
use schemas::SchemaType;
use serde_json::from_str;
use std::error::Error;
use term::Term;

use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

use crate::schemas::approve::AuditApproveSchema;
use crate::schemas::disapprove::AuditDisapproveSchema;
use crate::schemas::IntoTerm;

mod did;
mod error;
mod schemas;
mod term;
mod utils;

const MAX_TERM_BATCH_SIZE: u32 = 1000;
const MAX_ATT_BATCH_SIZE: u32 = 1000;
const ATTESTATION_SOURCE_ADDRESS: &str = "0x1";
const AUDIT_APPROVE_SCHEMA_ID: &str = "0x2";
const AUDIT_DISAPPROVE_SCHEMA_ID: &str = "0x3";
const STATUS_SCHEMA_ID: &str = "0x4";

#[derive(Debug)]
struct TransformerService {
	indexer_channel: Channel,
	lt_channel: Channel,
	db: String,
}

impl TransformerService {
	fn new(
		indexer_channel: Channel, lt_channel: Channel, db_url: &str,
	) -> Result<Self, AttTrError> {
		let db = DB::open_default(db_url).map_err(|x| AttTrError::DbError(x))?;
		let checkpoint = db.get(b"checkpoint").map_err(|x| AttTrError::DbError(x))?;
		if let None = checkpoint {
			let zero = 0u32.to_be_bytes();
			db.put(b"checkpoint", zero).map_err(|e| AttTrError::DbError(e))?;
			db.put(b"count", zero).map_err(|e| AttTrError::DbError(e))?;
		}

		Ok(Self { indexer_channel, lt_channel, db: db_url.to_string() })
	}

	fn read_checkpoint(db: &DB) -> Result<u32, AttTrError> {
		let offset_bytes_opt = db.get(b"checkpoint").map_err(|e| AttTrError::DbError(e))?;
		let offset_bytes = offset_bytes_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		let offset = u32::from_be_bytes(offset_bytes);
		Ok(offset)
	}

	fn write_checkpoint(db: &DB, checkpoint: u32, count: u32) -> Result<(), AttTrError> {
		db.put(b"checkpoint", checkpoint.to_be_bytes()).map_err(|e| AttTrError::DbError(e))?;
		db.put(b"count", count.to_be_bytes()).map_err(|e| AttTrError::DbError(e))?;
		Ok(())
	}

	fn read_terms(db: &DB, batch: TermBatch) -> Result<Vec<TermObject>, AttTrError> {
		let mut terms = Vec::new();
		for i in batch.start..batch.size {
			let id_bytes = i.to_be_bytes();
			let res_opt = db.get(id_bytes).map_err(|e| AttTrError::DbError(e))?;
			if let Some(res) = res_opt {
				if let Ok(term) = Term::from_bytes(res) {
					let term_obj: TermObject = term.into();
					terms.push(term_obj);
				}
			}
		}
		Ok(terms)
	}

	fn parse_event(event: IndexerEvent) -> Result<Vec<Term>, AttTrError> {
		let schema_id = event.schema_id;
		let schema_type = SchemaType::from(schema_id);
		let terms = match schema_type {
			SchemaType::AuditApprove => {
				let parsed_att: AuditApproveSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
			SchemaType::AuditDisapprove => {
				let parsed_att: AuditDisapproveSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
			SchemaType::StatusCredential => {
				let parsed_att: StatusSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
			SchemaType::TrustCredential => {
				let parsed_att: TrustSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::ParseError)?;
				parsed_att.into_term()?
			},
		};

		Ok(terms)
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
	async fn sync_indexer(&self, _: Request<Void>) -> Result<Response<Void>, Status> {
		let db = DB::open_default(self.db.clone())
			.map_err(|_| Status::internal("Failed to connect to DB"))?;

		let ch_offset = 0;
		let ct_offset = 0;

		let indexer_query = Query {
			source_address: ATTESTATION_SOURCE_ADDRESS.to_owned(),
			schema_id: vec![
				AUDIT_APPROVE_SCHEMA_ID.to_owned(),
				AUDIT_DISAPPROVE_SCHEMA_ID.to_owned(),
				STATUS_SCHEMA_ID.to_owned(),
			],
			offset: 0,
			count: MAX_ATT_BATCH_SIZE,
		};

		let mut client = IndexerClient::new(self.indexer_channel.clone());
		let mut response = client.subscribe(indexer_query).await?.into_inner();
		let mut checkpoint = ch_offset;
		let mut count = ct_offset;

		let mut terms = Vec::new();
		// ResponseStream
		while let Ok(Some(res)) = response.message().await {
			assert!(res.id == checkpoint + 1);
			let parsed_terms =
				Self::parse_event(res).map_err(|_| Status::internal("Failed to parse event"))?;
			terms.push(parsed_terms);
		}

		let num_new_term_groups =
			u32::try_from(terms.len()).map_err(|_| Status::internal("Failed to parse count"))?;
		checkpoint += num_new_term_groups;
		let (num_total_new_terms, indexed_terms): (u32, Vec<(u32, Term)>) =
			terms.iter().fold((0, Vec::new()), |(mut acc, mut new_items), items| {
				let indexed_items = items
					.into_iter()
					.map(|x| {
						let indexed_item = (acc, x.clone());
						acc += 1;
						indexed_item
					})
					.collect_vec();
				new_items.extend(indexed_items);
				(acc, new_items)
			});
		count += num_total_new_terms;
		println!("Received and saved terms: {:#?}", terms);

		Self::write_terms(&db, indexed_terms)
			.map_err(|_| Status::internal("Failed to write terms"))?;
		Self::write_checkpoint(&db, checkpoint, count)
			.map_err(|_| Status::internal("Failed to write checkpoint"))?;

		Ok(Response::new(Void::default()))
	}

	async fn term_stream(&self, request: Request<TermBatch>) -> Result<Response<Void>, Status> {
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

		let mut client = LinearCombinerClient::new(self.lt_channel.clone());
		let res = client.sync_transformer(Request::new(iter(terms))).await?;

		Ok(res)
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let indexer_channel = Channel::from_static("http://localhost:50050").connect().await?;
	let lt_channel = Channel::from_static("http://localhost:50052").connect().await?;
	let db_url = "att-tr-storage";
	let tr_service = TransformerService::new(indexer_channel, lt_channel, db_url)?;

	let addr = "[::1]:50051".parse()?;
	Server::builder().add_service(TransformerServer::new(tr_service)).serve(addr).await?;
	Ok(())
}

#[cfg(test)]
mod test {
	use crate::did::Did;
	use crate::schemas::status::{CredentialSubject, CurrentStatus, StatusSchema};
	use crate::schemas::{IntoTerm, Proof};
	use crate::utils::address_from_ecdsa_key;
	use crate::TransformerService;
	use itertools::Itertools;
	use proto_buf::indexer::IndexerEvent;
	use proto_buf::transformer::{TermBatch, TermObject};
	use rocksdb::DB;
	use secp256k1::rand::thread_rng;
	use secp256k1::{generate_keypair, Message, Secp256k1};
	use serde_json::to_string;
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_write_read_checkpoint() {
		let db = DB::open_default("att-tr-checkpoint-test-storage").unwrap();
		TransformerService::write_checkpoint(&db, 15, 14).unwrap();
		let checkpoint = TransformerService::read_checkpoint(&db).unwrap();
		assert_eq!(checkpoint, 15);
	}

	impl StatusSchema {
		pub fn generate(id: String, current_status: CurrentStatus) -> Self {
			let did = Did::parse_pkh_eth(id.clone()).unwrap();
			let mut keccak = Keccak256::new();
			keccak.update(&did.key);
			keccak.update(&[current_status.clone().into()]);
			let digest = keccak.finalize();

			let message = Message::from_digest_slice(digest.as_ref()).unwrap();

			let rng = &mut thread_rng();
			let (sk, pk) = generate_keypair(rng);
			let secp = Secp256k1::new();
			let res = secp.sign_ecdsa_recoverable(&message, &sk);
			let (rec_id, sig_bytes) = res.serialize_compact();
			let rec_id_i32 = rec_id.to_i32();

			let mut bytes = Vec::new();
			bytes.extend_from_slice(&sig_bytes);
			bytes.push(rec_id_i32.to_le_bytes()[0]);
			let encoded_sig = hex::encode(bytes);

			let kind = "StatusCredential".to_string();
			let addr = address_from_ecdsa_key(&pk);
			let issuer = format!("did:pkh:eth:{}", hex::encode(addr));
			let cs = CredentialSubject::new(id, current_status);
			let proof = Proof::new(encoded_sig);

			StatusSchema::new(kind, issuer, cs, proof)
		}
	}

	#[test]
	fn should_write_read_term() {
		let db = DB::open_default("att-tr-terms-test-storage").unwrap();

		let status_schema = StatusSchema::generate(
			"did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned(),
			CurrentStatus::Endorsed,
		);
		let indexed_event = IndexerEvent {
			id: 0,
			schema_id: 4,
			schema_value: to_string(&status_schema).unwrap(),
			timestamp: 2397848,
		};
		let terms = TransformerService::parse_event(indexed_event).unwrap();
		let indexed_terms = terms.into_iter().enumerate().map(|(i, x)| (i as u32, x)).collect_vec();
		TransformerService::write_terms(&db, indexed_terms).unwrap();

		let term_batch = TermBatch { start: 0, size: 1 };
		let terms = TransformerService::read_terms(&db, term_batch).unwrap();

		let status_terms = status_schema.into_term().unwrap();
		let term_objs: Vec<TermObject> = status_terms.into_iter().map(|x| x.into()).collect_vec();
		assert_eq!(terms, term_objs);
	}
}
