use std::error::Error;

use clap::Parser as ClapParser;
use futures::stream::iter;
use rocksdb::{Options, DB};
use serde_json::from_str;
use tonic::transport::Endpoint;
use tonic::{transport::Server, Request, Response, Status};

use error::AttTrError;
use managers::checkpoint::CheckpointManager;
use managers::term::TermManager;
use proto_buf::combiner::linear_combiner_client::LinearCombinerClient;
use proto_buf::indexer::indexer_client::IndexerClient;
use proto_buf::indexer::{IndexerEvent, Query};
use proto_buf::transformer::transformer_server::{Transformer, TransformerServer};
use proto_buf::transformer::{EventBatch, EventResult, TermBatch, TermResult};
// use schemas::security::SecurityReportSchema;
// use schemas::status::StatusSchema;
use schemas::trust::TrustSchema;
use schemas::{IntoTerm, SchemaType};
use term::Term;

pub mod did;
pub mod error;
pub mod managers;
pub mod schemas;
pub mod term;
pub mod utils;

const MAX_TERM_BATCH_SIZE: u32 = 1000;
const ATTESTATION_SOURCE_ADDRESS: &str = "0x1";
const AUDIT_APPROVE_SCHEMA_ID: &str = "0x2";
const AUDIT_DISAPPROVE_SCHEMA_ID: &str = "0x3";
const STATUS_SCHEMA_ID: &str = "0x4";

#[derive(Debug)]
struct TransformerService {
	indexer_endpoint: Endpoint,
	lt_endpoint: Endpoint,
	db_url: String,
}

impl TransformerService {
	fn new(
		indexer_endpoint: Endpoint, lt_endpoint: Endpoint, db_url: &str,
	) -> Result<Self, AttTrError> {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db =
			DB::open_cf(&opts, db_url, vec!["checkpoint", "term"]).map_err(AttTrError::DbError)?;
		CheckpointManager::init(&db)?;

		Ok(Self { indexer_endpoint, lt_endpoint, db_url: db_url.to_string() })
	}

	fn parse_event(event: IndexerEvent) -> Result<Vec<Term>, AttTrError> {
		let schema_id = event.schema_id;
		let schema_type = SchemaType::from(schema_id);
		let terms = match schema_type {
			SchemaType::SecurityCredential => {
				// TODO: Uncooment when security reports are included in LT
				// let parsed_att: SecurityReportSchema =
				// 	from_str(&event.schema_value).map_err(AttTrError::SerdeError)?;
				// parsed_att.into_term(event.timestamp)?
				Vec::new()
			},
			SchemaType::StatusCredential => {
				// TODO: Uncooment when snaps are included in LT
				// let parsed_att: StatusSchema =
				// 	from_str(&event.schema_value).map_err(AttTrError::SerdeError)?;
				// parsed_att.into_term(event.timestamp)?
				Vec::new()
			},
			SchemaType::TrustCredential => {
				let parsed_att: TrustSchema =
					from_str(&event.schema_value).map_err(AttTrError::SerdeError)?;
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

		let mut client = IndexerClient::connect(self.indexer_endpoint.clone())
			.await
			.map_err(|e| Status::from_error(Box::new(e)))?;
		let mut response = client.subscribe(indexer_query).await?.into_inner();

		let mut terms = Vec::new();
		// ResponseStream
		while let Ok(Some(res)) = response.message().await {
			let parsed_terms = Self::parse_event(res).map_err(|e| e.into_status())?;
			terms.push(parsed_terms);
		}
		println!("Received num events: {}", terms.len());
		println!("Received terms: {:#?}", terms);

		let num_new_term_groups =
			u32::try_from(terms.len()).map_err(|_| AttTrError::SerialisationError.into_status())?;
		let new_checkpoint = ch_offset + num_new_term_groups;

		let (new_count, indexed_terms) = TermManager::get_indexed_terms(ct_offset, terms)
			.map_err(|_| AttTrError::SerialisationError.into_status())?;

		println!("Received num terms: {}", new_count);

		TermManager::write_terms(&db, indexed_terms).map_err(|e| e.into_status())?;
		CheckpointManager::write_checkpoint(&db, new_checkpoint, new_count)
			.map_err(|e| e.into_status())?;

		let event_result = EventResult { num_terms: new_count - ct_offset, total_count: new_count };
		Ok(Response::new(event_result))
	}

	async fn term_stream(
		&self, request: Request<TermBatch>,
	) -> Result<Response<TermResult>, Status> {
		let inner = request.into_inner();
		if inner.size > MAX_TERM_BATCH_SIZE {
			return Err(Status::invalid_argument(format!(
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
		let num_terms = terms.len();

		let mut client = LinearCombinerClient::connect(self.lt_endpoint.clone())
			.await
			.map_err(|e| Status::from_error(Box::new(e)))?;
		client.sync_transformer(Request::new(iter(terms))).await?;

		let term_size =
			u32::try_from(num_terms).map_err(|_| AttTrError::SerialisationError.into_status())?;
		let res = TermResult { size: term_size };

		Ok(Response::new(res))
	}
}

#[derive(ClapParser)]
struct Args {
	/// Database (storage) directory.
	#[arg(long, default_value = "att-tr-storage")]
	db_dir: String,

	/// gRPC server listen address.
	#[arg(long, default_value = "[::1]:50051")]
	listen_address: std::net::SocketAddr,

	/// Indexer gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50050")]
	indexer_grpc: Endpoint,

	/// Linear combiner gRPC endpoint.
	#[arg(long, value_name = "URL", default_value = "http://[::1]:50052")]
	linear_combiner_grpc: Endpoint,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	let tr_service =
		TransformerService::new(args.indexer_grpc, args.linear_combiner_grpc, &args.db_dir)?;

	Server::builder()
		.add_service(TransformerServer::new(tr_service))
		.serve(args.listen_address)
		.await?;
	Ok(())
}

#[cfg(test)]
mod test {
	use secp256k1::rand::thread_rng;
	use secp256k1::{generate_keypair, Message, Secp256k1, SecretKey};
	use serde_json::to_string;
	use sha3::{Digest, Keccak256};

	use proto_buf::indexer::IndexerEvent;

	use crate::did::{Did, Schema};
	use crate::schemas::status::{CredentialSubject, CurrentStatus, StatusSchema};
	use crate::schemas::trust::{
		CredentialSubject as CredentialSubjectTrust, DomainTrust, TrustSchema,
	};
	use crate::schemas::{Domain, Proof};
	// use crate::term::{Term, TermForm};
	use crate::utils::address_from_ecdsa_key;
	use crate::TransformerService;

	impl StatusSchema {
		pub fn generate(id: String, current_status: CurrentStatus) -> Self {
			let did = Did::parse_snap(id.clone()).unwrap();
			let mut keccak = Keccak256::default();
			keccak.update([did.schema.into()]);
			keccak.update(&did.key);
			keccak.update([current_status.clone().into()]);
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
			let issuer = format!("did:pkh:eth:0x{}", hex::encode(addr));
			let cs = CredentialSubject::new(id, current_status, None);
			let proof = Proof::new(encoded_sig);

			StatusSchema::new(kind, issuer, cs, proof)
		}

		pub fn generate_from_sk(id: String, current_status: CurrentStatus, sk: SecretKey) -> Self {
			let did = Did::parse_snap(id.clone()).unwrap();
			let mut keccak = Keccak256::default();
			keccak.update([did.schema.into()]);
			keccak.update(&did.key);
			keccak.update([current_status.clone().into()]);
			let digest = keccak.finalize();

			let message = Message::from_digest_slice(digest.as_ref()).unwrap();

			let secp = Secp256k1::new();
			let pk = sk.public_key(&secp);

			let res = secp.sign_ecdsa_recoverable(&message, &sk);
			let (rec_id, sig_bytes) = res.serialize_compact();
			let rec_id_i32 = rec_id.to_i32();

			let mut bytes = Vec::new();
			bytes.extend_from_slice(&sig_bytes);
			bytes.push(rec_id_i32.to_le_bytes()[0]);
			let encoded_sig = hex::encode(bytes);

			let kind = "StatusCredential".to_string();
			let addr = address_from_ecdsa_key(&pk);
			let issuer = format!("did:pkh:eth:0x{}", hex::encode(addr));
			let cs = CredentialSubject::new(id, current_status, None);
			let proof = Proof::new(encoded_sig);

			StatusSchema::new(kind, issuer, cs, proof)
		}

		pub fn generate_from_sk_string(
			id: String, current_status: CurrentStatus, sk_string: String,
		) -> Self {
			let did = Did::parse_snap(id.clone()).unwrap();
			let mut keccak = Keccak256::default();
			keccak.update([did.schema.into()]);
			keccak.update(&did.key);
			keccak.update([current_status.clone().into()]);
			let digest = keccak.finalize();

			let message = Message::from_digest_slice(digest.as_ref()).unwrap();

			let secp = Secp256k1::new();
			let sk_bytes = hex::decode(sk_string).unwrap();
			let sk = SecretKey::from_slice(&sk_bytes).unwrap();
			let pk = sk.public_key(&secp);

			let res = secp.sign_ecdsa_recoverable(&message, &sk);
			let (rec_id, sig_bytes) = res.serialize_compact();
			let rec_id_i32 = rec_id.to_i32();

			let mut bytes = Vec::new();
			bytes.extend_from_slice(&sig_bytes);
			bytes.push(rec_id_i32.to_le_bytes()[0]);
			let encoded_sig = hex::encode(bytes);

			let kind = "StatusCredential".to_string();
			let addr = address_from_ecdsa_key(&pk);
			let issuer = format!("did:pkh:eth:0x{}", hex::encode(addr));
			let cs = CredentialSubject::new(id, current_status, None);
			let proof = Proof::new(encoded_sig);

			StatusSchema::new(kind, issuer, cs, proof)
		}
	}

	impl TrustSchema {
		pub fn generate_from_sk(did_string: String, trust_arc: DomainTrust, sk: SecretKey) -> Self {
			let did = Did::parse_pkh_eth(did_string.clone()).unwrap();

			let mut keccak = Keccak256::default();
			keccak.update([did.schema.into()]);
			keccak.update(&did.key);
			keccak.update([trust_arc.scope.clone().into()]);
			// keccak.update(&trust_arc.level.to_be_bytes());

			let digest = keccak.finalize();

			let message = Message::from_digest_slice(digest.as_ref()).unwrap();

			let secp = Secp256k1::new();
			let pk = sk.public_key(&secp);

			let res = secp.sign_ecdsa_recoverable(&message, &sk);
			let (rec_id, sig_bytes) = res.serialize_compact();
			let rec_id = rec_id.to_i32().to_le_bytes()[0];

			let mut bytes = Vec::new();
			bytes.extend_from_slice(&sig_bytes);
			bytes.push(rec_id);
			let sig_string = hex::encode(bytes);

			let kind = "TrustCredential".to_string();
			let addr = address_from_ecdsa_key(&pk);
			let issuer = format!("did:pkh:eth:0x{}", hex::encode(addr));
			let cs = CredentialSubjectTrust::new(did_string, vec![trust_arc]);
			let proof = Proof::new(sig_string);

			TrustSchema::new(kind, issuer, cs, proof)
		}

		pub fn generate_from_sk_string(
			did_string: String, trust_arc: DomainTrust, sk_string: String,
		) -> Self {
			let did = Did::parse_pkh_eth(did_string.clone()).unwrap();

			let mut keccak = Keccak256::default();
			keccak.update([did.schema.into()]);
			keccak.update(&did.key);
			keccak.update([trust_arc.scope.clone().into()]);
			// keccak.update(&trust_arc.level.to_be_bytes());

			let digest = keccak.finalize();

			let message = Message::from_digest_slice(digest.as_ref()).unwrap();

			let secp = Secp256k1::new();
			let sk_bytes = hex::decode(sk_string).unwrap();
			let sk = SecretKey::from_slice(&sk_bytes).unwrap();
			let pk = sk.public_key(&secp);

			let res = secp.sign_ecdsa_recoverable(&message, &sk);
			let (rec_id, sig_bytes) = res.serialize_compact();
			let rec_id = rec_id.to_i32().to_le_bytes()[0];

			let mut bytes = Vec::new();
			bytes.extend_from_slice(&sig_bytes);
			bytes.push(rec_id);
			let sig_string = hex::encode(bytes);

			let kind = "TrustCredential".to_string();
			let addr = address_from_ecdsa_key(&pk);
			let issuer = format!("did:pkh:eth:0x{}", hex::encode(addr));
			let cs = CredentialSubjectTrust::new(did_string, vec![trust_arc]);
			let proof = Proof::new(sig_string);

			TrustSchema::new(kind, issuer, cs, proof)
		}
	}

	// TODO(ek): Reinstate with an instance of TrustSchema,
	//   or when we re-enable StatusSchema for security reports.
	// #[test]
	// fn should_parse_event() {
	// 	let recipient = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
	// 	let status_schema = StatusSchema::generate(recipient.clone(), CurrentStatus::Endorsed);
	// 	let timestamp = 2397848;
	// 	let indexed_event = IndexerEvent {
	// 		id: 0,
	// 		schema_id: 1,
	// 		schema_value: to_string(&status_schema).unwrap(),
	// 		timestamp,
	// 	};
	// 	let terms = TransformerService::parse_event(indexed_event).unwrap();
	// 	assert_eq!(
	// 		terms,
	// 		vec![Term::new(
	// 			status_schema.get_issuer(),
	// 			recipient,
	// 			50.,
	// 			Domain::SoftwareSecurity.into(),
	// 			TermForm::Trust,
	// 			timestamp,
	// 		)]
	// 	)
	// }

	fn print_csv(
		trust_arcs: Vec<TrustSchema>, status_arcs: Vec<StatusSchema>, mut id: u32, timestamp: u64,
	) -> u32 {
		let trust_schema_id = 2;
		let status_schema_id = 1;

		for schema_value in trust_arcs {
			// Validate event
			let indexed_event = IndexerEvent {
				id,
				schema_id: trust_schema_id,
				schema_value: to_string(&schema_value).unwrap(),
				timestamp,
			};
			let _ = TransformerService::parse_event(indexed_event).unwrap();

			let string = [
				id.to_string(),
				timestamp.to_string(),
				trust_schema_id.to_string(),
				to_string(&schema_value).unwrap(),
			]
			.join(";");
			println!("{}", string);

			id += 1;
		}

		for schema_value in status_arcs {
			// Validate event
			let indexed_event = IndexerEvent {
				id,
				schema_id: status_schema_id,
				schema_value: to_string(&schema_value).unwrap(),
				timestamp,
			};
			let _ = TransformerService::parse_event(indexed_event).unwrap();

			let string = [
				id.to_string(),
				timestamp.to_string(),
				status_schema_id.to_string(),
				to_string(&schema_value).unwrap(),
			]
			.join(";");
			println!("{}", string);

			id += 1;
		}

		id
	}

	#[test]
	fn generate_functional_test_schemas() {
		let x_sk = "7f6f2ccdb23f2abb7b69278e947c01c6160a31cf02c19d06d0f6e5ab1d768b95".to_owned();
		let x = "did:pkh:eth:0xa9572220348b1080264e81c0779f77c144790cd6".to_owned();

		let y_sk = "117be1de549d1d4322c4711f11efa0c5137903124f85fc37c761ffc91ace30cb".to_owned();
		let y = "did:pkh:eth:0xba9090181312bd0e40254a3dc29841980dd392d2".to_owned();

		let z_sk = "ac7f0d9eaea4d4bf5438b887e34d0cf87e7f98d97da70eff001850487b2cae23".to_owned();
		let z = "did:pkh:eth:0x9a2954b87d8745df0b1010291c51d68ae9269d43".to_owned();

		let p_sk = "bbb7d40b7bb8e41c550696fdef78fff6f013bb34627ba50ca2d63b6e84cffa6c".to_owned();
		let _p = "did:pkh:eth:0x651a3c584f4c71b54c50ea73f41b936845ab4fdf".to_owned();

		let q_sk = "9a32e1a6638ce87528a3f0303c7a9cecba4ed5fef0551f3afd1c7865bc66308f".to_owned();
		let _q = "did:pkh:eth:0x138aaabbc2ad61f8ea7f2d4155cc7323f26f8775".to_owned();

		let s1 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1".to_owned();
		let s2 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();

		// Trust
		// p => x - Trust Credential - Honesty - trust
		// x => z - Trust credential - Honesty - trust
		// p => x - Trust Credential - Software security - trust
		// q => y - Trust Credential - Software security - trust
		// p => s1 - Status Credential - Endorse
		// q => s2 - Status Credential - Endorse
		// x => s1 - Status Credential - Endorse

		let p_x1 = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::Honesty, 1., Vec::new()),
			p_sk.clone(),
		);
		let x_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::Honesty, 1., Vec::new()),
			x_sk.clone(),
		);

		let p_x2 = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			p_sk.clone(),
		);
		let q_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			q_sk.clone(),
		);

		let q_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			q_sk.clone(),
		);
		let p_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Endorsed,
			p_sk.clone(),
		);
		let x_s1 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			x_sk.clone(),
		);

		// Distrust
		// p => y - Trust Credential - Honest - distrust
		// q => x - Trust Credential - Software security - distrust
		// y => z - Trust Credential - Software security - distrust
		// y => s2 - Status Credential - Dispute
		// z => s1 - Status Credential - Dispute
		// z => s2 - Status Credential - Dispute
		let p_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::Honesty, -1., Vec::new()),
			p_sk.clone(),
		);
		let q_x = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			q_sk.clone(),
		);
		let y_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			y_sk.clone(),
		);

		let y_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Disputed,
			y_sk.clone(),
		);
		let z_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Disputed,
			z_sk.clone(),
		);
		let z_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Disputed,
			z_sk.clone(),
		);

		let trust_arcs = [p_x1, p_x2, q_y, x_z, p_y, q_x, y_z];
		let status_arcs = [q_s2, p_s1, x_s1, y_s2, z_s1, z_s2];

		println!("num attestations: {}", trust_arcs.len() + status_arcs.len());

		let timestamp = 2397848;
		let id = 1;
		println!("id;timestamp;schema_id;schema_value");
		print_csv(trust_arcs.to_vec(), status_arcs.to_vec(), id, timestamp);
	}

	#[test]
	fn generate_sybil_attack_test_schemas() {
		let x_sk = "7f6f2ccdb23f2abb7b69278e947c01c6160a31cf02c19d06d0f6e5ab1d768b95".to_owned();
		let x = "did:pkh:eth:0xa9572220348b1080264e81c0779f77c144790cd6".to_owned();

		let y_sk = "117be1de549d1d4322c4711f11efa0c5137903124f85fc37c761ffc91ace30cb".to_owned();
		let y = "did:pkh:eth:0xba9090181312bd0e40254a3dc29841980dd392d2".to_owned();

		let z_sk = "ac7f0d9eaea4d4bf5438b887e34d0cf87e7f98d97da70eff001850487b2cae23".to_owned();
		let z = "did:pkh:eth:0x9a2954b87d8745df0b1010291c51d68ae9269d43".to_owned();

		let p_sk = "bbb7d40b7bb8e41c550696fdef78fff6f013bb34627ba50ca2d63b6e84cffa6c".to_owned();
		let p = "did:pkh:eth:0x651a3c584f4c71b54c50ea73f41b936845ab4fdf".to_owned();

		let q_sk = "9a32e1a6638ce87528a3f0303c7a9cecba4ed5fef0551f3afd1c7865bc66308f".to_owned();
		let _q = "did:pkh:eth:0x138aaabbc2ad61f8ea7f2d4155cc7323f26f8775".to_owned();

		let s1 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1".to_owned();
		let s2 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();

		// Trust - Direct
		// x => y - Trust Credential - Software security - trust
		// x => z - Trust Credential - Software security - trust
		// y => x - Trust Credential - Software security - trust
		// y => z - Trust Credential - Software security - trust
		// z => x - Trust Credential - Software security - trust
		// z => y - Trust Credential - Software security - trust
		// q => y - Trust Credential - Software security - trust
		let x_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			x_sk.clone(),
		);
		let x_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			x_sk.clone(),
		);
		let y_x = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			y_sk.clone(),
		);
		let y_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			y_sk.clone(),
		);
		let z_x = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			z_sk.clone(),
		);
		let z_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			z_sk.clone(),
		);
		let q_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			q_sk.clone(),
		);

		// Trust - Snap
		// x => s1 - Status Credential - Endorse
		// y => s1 - Status Credential - Endorse
		// z => s1 - Status Credential - Endorse
		// p => s2 - Status Credential - Endorse
		// q => s2 - Status Credential - Endorse
		let x_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Endorsed,
			x_sk.clone(),
		);
		let y_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Endorsed,
			y_sk.clone(),
		);
		let z_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Endorsed,
			z_sk.clone(),
		);
		let p_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			p_sk.clone(),
		);
		let q_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			q_sk.clone(),
		);

		// Distrust - Direct
		// p => x - Trust Credential - Software security - distrust
		// p => y - Trust Credential - Software security - distrust
		// p => z - Trust Credential - Software security - distrust
		// x => p - Trust Credential - Software security - distrust
		// y => p - Trust Credential - Software security - distrust
		// z => p - Trust Credential - Software security - distrust
		let p_x = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			p_sk.clone(),
		);
		let p_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			p_sk.clone(),
		);
		let p_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			p_sk.clone(),
		);
		let x_p = TrustSchema::generate_from_sk_string(
			p.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			x_sk.clone(),
		);
		let y_p = TrustSchema::generate_from_sk_string(
			p.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			y_sk.clone(),
		);
		let z_p = TrustSchema::generate_from_sk_string(
			p.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			z_sk.clone(),
		);

		// Distrust - Snap
		// p => s1 - Status Credential - Dispute
		// q => s1 - Status Credential - Dispute
		let p_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Disputed,
			p_sk.clone(),
		);
		let q_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Disputed,
			q_sk.clone(),
		);

		let trust_arcs = [x_y, x_z, y_x, y_z, z_x, z_y, q_y, p_x, p_y, p_z, x_p, y_p, z_p];
		let status_arcs = [x_s1, y_s1, z_s1, p_s2, q_s2, p_s1, q_s1];

		println!("num attestations: {}", trust_arcs.len() + status_arcs.len());

		let timestamp = 2397848;
		let id = 1;
		println!("id;timestamp;schema_id;schema_value");
		print_csv(trust_arcs.to_vec(), status_arcs.to_vec(), id, timestamp);
	}

	#[test]
	fn generate_sleeping_agent_attack_test_schemas() {
		let _x_sk = "7f6f2ccdb23f2abb7b69278e947c01c6160a31cf02c19d06d0f6e5ab1d768b95".to_owned();
		let x = "did:pkh:eth:0xa9572220348b1080264e81c0779f77c144790cd6".to_owned();

		let _y_sk = "117be1de549d1d4322c4711f11efa0c5137903124f85fc37c761ffc91ace30cb".to_owned();
		let y = "did:pkh:eth:0xba9090181312bd0e40254a3dc29841980dd392d2".to_owned();

		let z_sk = "ac7f0d9eaea4d4bf5438b887e34d0cf87e7f98d97da70eff001850487b2cae23".to_owned();
		let z = "did:pkh:eth:0x9a2954b87d8745df0b1010291c51d68ae9269d43".to_owned();

		let p_sk = "bbb7d40b7bb8e41c550696fdef78fff6f013bb34627ba50ca2d63b6e84cffa6c".to_owned();
		let p = "did:pkh:eth:0x651a3c584f4c71b54c50ea73f41b936845ab4fdf".to_owned();

		let q_sk = "9a32e1a6638ce87528a3f0303c7a9cecba4ed5fef0551f3afd1c7865bc66308f".to_owned();
		let q = "did:pkh:eth:0x138aaabbc2ad61f8ea7f2d4155cc7323f26f8775".to_owned();

		let s1 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1".to_owned();
		let s2 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();

		// Trust - Direct
		// P => Q - Trust Credential - Software security - trust
		// Q => P - Trust Credential - Software security - trust
		// P => Z - Trust Credential - Software security - trust
		// Q => Z - Trust Credential - Software security - trust
		let p_q = TrustSchema::generate_from_sk_string(
			q.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			p_sk.clone(),
		);
		let q_p = TrustSchema::generate_from_sk_string(
			p.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			q_sk.clone(),
		);
		let p_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			p_sk.clone(),
		);
		let q_z = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
			q_sk.clone(),
		);

		// Trust - Snap
		// P => S2 - Status Credential - Endorse
		// Q => S2 - Status Credential - Endorse
		// Z => S2 - Status Credential - Endorse
		let p_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			p_sk.clone(),
		);
		let q_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			q_sk.clone(),
		);
		let z_s2 = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Endorsed,
			z_sk.clone(),
		);

		// Distrust - Direct
		// P => X - Trust Credential - Software security - distrust
		// P => Y - Trust Credential - Software security - distrust
		// Q => X - Trust Credential - Software security - distrust
		// Q => Y - Trust Credential - Software security - distrust
		let p_x = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			p_sk.clone(),
		);
		let p_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			p_sk.clone(),
		);
		let q_x = TrustSchema::generate_from_sk_string(
			x.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			q_sk.clone(),
		);
		let q_y = TrustSchema::generate_from_sk_string(
			y.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			q_sk.clone(),
		);

		// Distrust - Snap
		// Z => S2 - Status Credential - Dispute
		let z_s2_override = StatusSchema::generate_from_sk_string(
			s2.clone(),
			CurrentStatus::Disputed,
			z_sk.clone(),
		);

		// Trust - Snap
		// Z => S1 - Status Credential - Endorse
		let z_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Endorsed,
			z_sk.clone(),
		);

		// Distrust - Direct
		// P => Z - Trust Credential - Software security - distrust
		// Q => Z - Trust Credential - Software security - distrust
		let p_z_2 = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			p_sk.clone(),
		);
		let q_z_2 = TrustSchema::generate_from_sk_string(
			z.clone(),
			DomainTrust::new(Domain::SoftwareSecurity, -1., Vec::new()),
			q_sk.clone(),
		);

		// Distrust - Snap
		// P => S1 - Status Credential - Dispute
		// Q => S1 - Status Credential - Dispute
		let p_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Disputed,
			p_sk.clone(),
		);
		let q_s1 = StatusSchema::generate_from_sk_string(
			s1.clone(),
			CurrentStatus::Disputed,
			q_sk.clone(),
		);

		// 1st round
		let trust_arcs_1st = [p_q, q_p, p_z, q_z, p_x, p_y, q_x, q_y];
		let status_arcs_1st = [p_s2, q_s2, z_s2];
		// 2nd round
		let status_arcs_2nd = [z_s1, z_s2_override];
		// 3rd round
		let trust_arcs_3rd = [p_z_2, q_z_2];
		let status_arcs_3rd = [p_s1, q_s1];

		println!(
			"num attestations: {}",
			trust_arcs_1st.len() + status_arcs_1st.len() + status_arcs_2nd.len()
		);

		println!("id;timestamp;schema_id;schema_value");
		let mut timestamp = 2397848;
		let id = 1;
		let id = print_csv(
			trust_arcs_1st.to_vec(),
			status_arcs_1st.to_vec(),
			id,
			timestamp,
		);
		timestamp += 610000;
		let id = print_csv(Vec::new(), status_arcs_2nd.to_vec(), id, timestamp);
		timestamp += 610000;
		print_csv(
			trust_arcs_3rd.to_vec(),
			status_arcs_3rd.to_vec(),
			id,
			timestamp,
		);
	}

	#[test]
	fn generate_100_sybils_test_schemas() {
		let s1 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let s2 = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1".to_owned();

		let num_trustees = 20;
		let rng = &mut thread_rng();
		let secp = Secp256k1::new();

		let mut trustees = Vec::new();
		let mut sks = Vec::new();
		trustees.push("did:pkh:eth:0x90f8bf6a479f320ead074411a4b0e7944ea8c9c5".to_string());

		let mut trust_credentials = Vec::new();
		let mut status_credentials = Vec::new();
		for _ in 0..num_trustees {
			let sk = SecretKey::new(rng);
			sks.push(sk);

			for trustee in &trustees {
				let trust_credential = TrustSchema::generate_from_sk(
					trustee.clone(),
					DomainTrust::new(Domain::SoftwareSecurity, 1., Vec::new()),
					sk,
				);
				trust_credentials.push(trust_credential);
			}

			let pk = sk.public_key(&secp);
			let addr = address_from_ecdsa_key(&pk);
			let did = Did::new(Schema::PkhEth, addr);
			let did_string: String = did.into();
			trustees.push(did_string);

			let endorsment_credential =
				StatusSchema::generate_from_sk(s1.clone(), CurrentStatus::Endorsed, sk);
			let dispute_credential =
				StatusSchema::generate_from_sk(s2.clone(), CurrentStatus::Disputed, sk);
			status_credentials.push(endorsment_credential);
			status_credentials.push(dispute_credential);
		}

		println!(
			"num attestations: {}",
			trust_credentials.len() + status_credentials.len()
		);

		let timestamp = 2397848;
		let id = 1;
		println!("id;timestamp;schema_id;schema_value");
		print_csv(
			trust_credentials.to_vec(),
			status_credentials.to_vec(),
			id,
			timestamp,
		);
	}
}
