use error::AttTrError;
use futures::stream::iter;
use managers::checkpoint::CheckpointManager;
use managers::term::TermManager;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::common::Void;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::{IndexerEvent, Query};
use proto_buf::transformer::transformer_server::{Transformer, TransformerServer};
use proto_buf::transformer::{EventBatch, EventResult, TermBatch};
use rocksdb::{Options, DB};
use schemas::security::SecurityReportSchema;
use schemas::status::StatusSchema;
use schemas::trust::TrustSchema;
use schemas::{IntoTerm, SchemaType};
use serde_json::from_str;
use std::error::Error;
use term::Term;

use tonic::transport::Channel;
use tonic::{transport::Server, Request, Response, Status};

mod did;
mod error;
mod managers;
mod schemas;
mod term;
mod utils;

const MAX_TERM_BATCH_SIZE: u32 = 1000;
const ATTESTATION_SOURCE_ADDRESS: &str = "0x1";
const AUDIT_APPROVE_SCHEMA_ID: &str = "0x2";
const AUDIT_DISAPPROVE_SCHEMA_ID: &str = "0x3";
const STATUS_SCHEMA_ID: &str = "0x4";

#[derive(Debug)]
struct TransformerService {
	indexer_channel: Channel,
	lt_channel: Channel,
	db_url: String,
}

impl TransformerService {
	fn new(
		indexer_channel: Channel, lt_channel: Channel, db_url: &str,
	) -> Result<Self, AttTrError> {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, db_url, vec!["checkpoint", "term"])
			.map_err(|e| AttTrError::DbError(e))?;
		CheckpointManager::init(&db)?;

		Ok(Self { indexer_channel, lt_channel, db_url: db_url.to_string() })
	}

	fn parse_event(event: IndexerEvent) -> Result<Vec<Term>, AttTrError> {
		let schema_id = event.schema_id;
		let schema_type = SchemaType::from(schema_id);
		let terms = match schema_type {
			SchemaType::SecurityCredential => {
				let parsed_att: SecurityReportSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::SerdeError(e))?;
				parsed_att.into_term(event.timestamp)?
			},
			SchemaType::StatusCredential => {
				let parsed_att: StatusSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::SerdeError(e))?;
				parsed_att.into_term(event.timestamp)?
			},
			SchemaType::TrustCredential => {
				let parsed_att: TrustSchema =
					from_str(&event.schema_value).map_err(|e| AttTrError::SerdeError(e))?;
				parsed_att.into_term(event.timestamp)?
			},
		};

		Ok(terms)
	}
}

#[tonic::async_trait]
impl Transformer for TransformerService {
	async fn sync_indexer(
		&self, req: Request<EventBatch>,
	) -> Result<Response<EventResult>, Status> {
		let event_batch = req.into_inner();
		if event_batch.size == 0 {
			return Err(Status::invalid_argument("Invalid `size`."));
		}

		let db = DB::open_cf(
			&Options::default(),
			&self.db_url,
			vec!["term", "checkpoint"],
		)
		.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let (ch_offset, ct_offset) =
			CheckpointManager::read_checkpoint(&db).map_err(|e| e.into_status())?;

		let indexer_query = Query {
			source_address: ATTESTATION_SOURCE_ADDRESS.to_owned(),
			schema_id: vec![
				AUDIT_APPROVE_SCHEMA_ID.to_owned(),
				AUDIT_DISAPPROVE_SCHEMA_ID.to_owned(),
				STATUS_SCHEMA_ID.to_owned(),
			],
			offset: ch_offset,
			count: event_batch.size,
		};

		let mut client = IndexerClient::new(self.indexer_channel.clone());
		let mut response = client.subscribe(indexer_query).await?.into_inner();

		let mut terms = Vec::new();
		// ResponseStream
		while let Ok(Some(res)) = response.message().await {
			let parsed_terms = Self::parse_event(res).map_err(|e| e.into_status())?;
			terms.push(parsed_terms);
		}
		println!("Received terms: {:#?}", terms);

		let num_new_term_groups =
			u32::try_from(terms.len()).map_err(|_| AttTrError::SerialisationError.into_status())?;
		let new_checkpoint = ch_offset + num_new_term_groups;

		let (new_count, indexed_terms) = TermManager::get_indexed_terms(ct_offset, terms)
			.map_err(|_| AttTrError::SerialisationError.into_status())?;

		TermManager::write_terms(&db, indexed_terms).map_err(|e| e.into_status())?;
		CheckpointManager::write_checkpoint(&db, new_checkpoint, new_count)
			.map_err(|e| e.into_status())?;

		let event_result = EventResult { num_terms: new_count - ct_offset, total_count: new_count };
		Ok(Response::new(event_result))
	}

	async fn term_stream(&self, request: Request<TermBatch>) -> Result<Response<Void>, Status> {
		let inner = request.into_inner();
		if inner.size > MAX_TERM_BATCH_SIZE {
			return Result::Err(Status::invalid_argument(format!(
				"Batch size too big. Max size: {}",
				MAX_TERM_BATCH_SIZE
			)));
		}

		let db = DB::open_cf(
			&Options::default(),
			&self.db_url,
			vec!["term", "checkpoint"],
		)
		.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let terms = TermManager::read_terms(&db, inner).map_err(|e| e.into_status())?;

		let mut client = LinearCombinerClient::new(self.lt_channel.clone());
		let res = client.sync_transformer(Request::new(iter(terms))).await?;

		Ok(res)
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let indexer_channel = Channel::from_static("http://localhost:50050").connect().await?;
	let lc_channel = Channel::from_static("http://localhost:50052").connect().await?;
	let db_url = "att-tr-storage";
	let tr_service = TransformerService::new(indexer_channel, lc_channel, db_url)?;

	let addr = "[::1]:50051".parse()?;
	Server::builder().add_service(TransformerServer::new(tr_service)).serve(addr).await?;
	Ok(())
}

#[cfg(test)]
mod test {
	use crate::did::Did;
	use crate::schemas::status::{CredentialSubject, CurrentStatus, StatusSchema};
	use crate::schemas::{Domain, Proof};
	use crate::term::Term;
	use crate::utils::address_from_ecdsa_key;
	use crate::TransformerService;
	use proto_buf::indexer::IndexerEvent;
	use secp256k1::rand::thread_rng;
	use secp256k1::{generate_keypair, Message, Secp256k1};
	use serde_json::to_string;
	use sha3::{Digest, Keccak256};

	impl StatusSchema {
		pub fn generate(id: String, current_status: CurrentStatus) -> Self {
			let did = Did::parse_pkh_eth(id.clone()).unwrap();
			let mut keccak = Keccak256::default();
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
	fn should_parse_event() {
		let recipient = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let status_schema = StatusSchema::generate(recipient.clone(), CurrentStatus::Endorsed);
		let timestamp = 2397848;
		let indexed_event = IndexerEvent {
			id: 0,
			schema_id: 4,
			schema_value: to_string(&status_schema).unwrap(),
			timestamp,
		};
		let terms = TransformerService::parse_event(indexed_event).unwrap();
		assert_eq!(
			terms,
			vec![Term::new(
				status_schema.get_issuer(),
				recipient,
				25.,
				Domain::SoftwareSecurity.into(),
				true,
				timestamp,
			)]
		)
	}
}
