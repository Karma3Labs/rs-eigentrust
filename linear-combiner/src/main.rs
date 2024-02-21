use std::error::Error;

use clap::Parser as ClapParser;
use rocksdb::{Options, DB};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tracing::{debug, info};

use error::LcError;
use managers::{
	checkpoint::CheckpointManager, index::IndexManager, item::ItemManager, mapping::MappingManager,
	update::UpdateManager,
};
use proto_buf::{
	combiner::{
		linear_combiner_server::{LinearCombiner, LinearCombinerServer},
		LtBatch, LtHistoryBatch, LtObject, Mapping, MappingQuery,
	},
	common::Void,
	transformer::TermObject,
};

pub mod error;
pub mod item;
pub mod managers;

#[derive(Clone, Debug)]
struct LinearCombinerService {
	db_url: String,
}

impl LinearCombinerService {
	pub fn new(db_url: &str) -> Result<Self, LcError> {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(
			&opts,
			db_url,
			vec!["checkpoint", "index", "item", "mapping", "update"],
		)
		.map_err(LcError::DbError)?;
		CheckpointManager::init(&db)?;
		let mut offset = CheckpointManager::read_checkpoint(&db)?;
		// TODO(ek): Turn into CLI parameters.
		for did in vec![
			"did:pkh:eip155:59144:0x44dc4e3309b80ef7abf41c7d0a68f0337a88f044",
			"did:pkh:eip155:59144:0x4EBee6bA2771C19aDf9AF348985bCf06d3270d42",
			"did:pkh:eip155:59144:0xe77162b7D2CEb3625a4993Bab557403a7B706F18",
			"did:pkh:eip155:59144:0x47170ceaE335a9db7e96B72de630389669b33471",
			"did:pkh:eip155:59144:0xa2e73c800aE76d506d46e002cB14d1D4A08D3199",
			"did:pkh:eip155:59144:0x982ae6031EBE31e1A01490dd4D3270003d732830",
			"did:pkh:eip155:59144:0x932F1d969F13D314f3d0A234a3FFcc88372CDFf1",
			"did:pkh:eip155:59144:0x02c15D12240E1dFE098F89E6Ef9eF5BC4e477025",
			"did:pkh:eip155:59144:0xE5aF1B8619E3FbD91aFDFB710b0cF688Ce1a4fCF",
			"did:pkh:eip155:59144:0xfA045B2F2A25ad0B7365010eaf9AC2Dd9905895c",
			"did:pkh:eip155:59144:0x10A772110e02BaA56BeF4A56778F3E692E4373ac",
			"did:pkh:eip155:59144:0x4a12D8389696eff9294DEcE42A648588eda0F56d",
			"did:pkh:eip155:59144:0x23d86aA31D4198A78Baa98e49bb2dA52CD15c6f0",
			"did:pkh:eip155:59144:0xE5aF1B8619E3FbD91aFDFB710b0cF688Ce1a4fCF",
			"did:pkh:eip155:59144:0x6eCfD8252C19aC2Bf4bd1cBdc026C001C93E179D",
			"did:pkh:eip155:59144:0x224b11F0747c7688a10aCC15F785354aA6493ED6",
			"did:pkh:eip155:59144:0xd14BF29e486DFC3836757b9B8CCFc95a5160A56D",
			"did:pkh:eip155:59144:0x65a4CeC9f1c6060f3b987d9332Bedf26e8E86D17",
			"did:pkh:eip155:59144:0x8Ef9328D63203230a295FA9Bf9fCd8C5384349C2",
			"did:pkh:eip155:59144:0x3959ae2c154C443fc744861b6140dA6C8c97a3e3",
		] {
			get_index(&db, did, &mut offset)?;
		}
		CheckpointManager::write_checkpoint(&db, offset)?;

		Ok(Self { db_url: db_url.to_string() })
	}
}

fn get_index(db: &DB, did: &str, offset: &mut u32) -> Result<[u8; 4], LcError> {
	let did = if did.to_lowercase().starts_with("did:pkh:eip155:") {
		// Erase chain ID, de-checksum to lowercase
		let components: Vec<&str> = did.split(':').collect();
		format!("did:pkh:eip155:1:{}", components[4])
	} else {
		did.to_string()
	};
	let (idx, is_new) = IndexManager::get_index(db, did.to_lowercase(), *offset)?;
	if is_new {
		MappingManager::write_mapping(db, idx.to_vec(), did.clone())?;
		*offset += 1;
	}
	Ok(idx)
}

#[tonic::async_trait]
impl LinearCombiner for LinearCombinerService {
	async fn sync_transformer(
		&self, request: Request<Streaming<TermObject>>,
	) -> Result<Response<Void>, Status> {
		let db = DB::open_cf(
			&Options::default(),
			&self.db_url,
			vec!["checkpoint", "index", "item", "mapping", "update"],
		)
		.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mut offset = CheckpointManager::read_checkpoint(&db)?;

		let mut terms = Vec::new();
		let mut stream = request.into_inner();
		while let Some(term) = stream.message().await? {
			terms.push(term);
		}

		for term in terms {
			let domain = term.domain.to_be_bytes();
			let form = term.form.to_be_bytes();

			let x = get_index(&db, &term.from, &mut offset)?;
			let y = get_index(&db, &term.to, &mut offset)?;

			let timestamp = chrono::NaiveDateTime::from_timestamp_millis(term.timestamp as i64)
				.unwrap()
				.and_utc()
				.to_rfc3339();

			let mut key = Vec::new();
			key.extend_from_slice(&domain);
			key.extend_from_slice(&form);
			key.extend_from_slice(&x);
			key.extend_from_slice(&y);

			debug!(
				timestamp,
				term.from,
				x = u32::from_be_bytes(x),
				term.to,
				y = u32::from_be_bytes(y),
				term.domain,
				term.form,
				term.weight,
				"received item"
			);

			let value = ItemManager::update_value(&db, key.clone(), term.weight, term.timestamp)?;
			UpdateManager::set_value(&db, key.clone(), value, term.timestamp)?;
		}

		CheckpointManager::write_checkpoint(&db, offset)?;

		Ok(Response::new(Void {}))
	}
	type GetDidMappingStream = ReceiverStream<Result<Mapping, Status>>;
	async fn get_did_mapping(
		&self, request: Request<MappingQuery>,
	) -> Result<Response<Self::GetDidMappingStream>, Status> {
		let mapping_query = request.into_inner();
		let db =
			DB::open_cf_for_read_only(&Options::default(), &self.db_url, vec!["mapping"], false)
				.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mappings = MappingManager::read_mappings(&db, mapping_query.start, mapping_query.size)?;

		let (tx, rx) = channel(4);
		tokio::spawn(async move {
			for x in mappings.clone() {
				let x_obj: Mapping = x.into();
				tx.send(Ok(x_obj)).await.unwrap();
			}
		});

		Ok(Response::new(ReceiverStream::new(rx)))
	}

	type GetNewDataStream = ReceiverStream<Result<LtObject, Status>>;

	async fn get_new_data(
		&self, request: Request<LtBatch>,
	) -> Result<Response<Self::GetNewDataStream>, Status> {
		let batch = request.into_inner();
		let db = DB::open_cf(
			&Options::default(),
			&self.db_url,
			vec!["checkpoint", "index", "item", "mapping", "update"],
		)
		.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mut prefix = Vec::new();
		prefix.extend_from_slice(&batch.domain.to_be_bytes());
		prefix.extend_from_slice(&batch.form.to_be_bytes());
		let items = UpdateManager::read_batch(&db, prefix.clone(), batch.size)?;

		let items_to_send = items.clone();
		let (tx, rx) = channel(4);
		tokio::spawn(async move {
			for x in items_to_send {
				let x_obj: LtObject = x.into();
				tx.send(Ok(x_obj)).await.unwrap();
			}
		});

		// TODO: Uncomment when fixed
		// UpdateManager::delete_batch(&db, prefix, items)?;

		Ok(Response::new(ReceiverStream::new(rx)))
	}

	type GetHistoricDataStream = ReceiverStream<Result<LtObject, Status>>;

	async fn get_historic_data(
		&self, request: Request<LtHistoryBatch>,
	) -> Result<Response<Self::GetHistoricDataStream>, Status> {
		let batch = request.into_inner();
		let db = DB::open_cf_for_read_only(&Options::default(), &self.db_url, vec!["item"], false)
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let is_x_bigger = batch.x0 <= batch.x1;
		let is_y_bigger = batch.y0 <= batch.y1;
		if !is_x_bigger && !is_y_bigger {
			return Err(Status::invalid_argument("Invalid points!"));
		}

		let domain_bytes = batch.domain.to_be_bytes();
		let form_bytes = batch.form.to_be_bytes();

		let x_start = batch.x0;
		let x_end = batch.x1;

		let y_start = batch.y0;
		let y_end = batch.y1;

		let mut prefix = Vec::new();
		prefix.extend_from_slice(&domain_bytes);
		prefix.extend_from_slice(&form_bytes);

		let items = ItemManager::read_window(&db, prefix, (x_start, y_start), (x_end, y_end))?;

		// trace!(?items, "read items");

		let (tx, rx) = channel(4);
		tokio::spawn(async move {
			for x in items.clone() {
				let x_obj: LtObject = x.into();
				tx.send(Ok(x_obj)).await.unwrap();
			}
		});

		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

#[derive(ClapParser)]
struct Args {
	/// Database (storage) directory
	#[arg(long, default_value = "lc-storage")]
	db_dir: String,

	/// gRPC server listen address
	#[arg(long, default_value = "[::1]:50052")]
	listen_address: std::net::SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	{
		use tracing_subscriber::*;
		let env_filter = EnvFilter::builder()
			.with_env_var("SPD_LC_LOG")
			.from_env()?
			.add_directive(filter::LevelFilter::WARN.into());
		fmt::Subscriber::builder()
			.with_env_filter(env_filter)
			.with_writer(std::io::stderr)
			.with_ansi(true)
			.init();
	}
	info!("initializing LC");
	let service = LinearCombinerService::new(&args.db_dir)?;
	Server::builder()
		.add_service(LinearCombinerServer::new(service))
		.serve(args.listen_address)
		.await?;
	Ok(())
}
