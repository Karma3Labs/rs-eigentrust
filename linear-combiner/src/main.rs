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
use rocksdb::{Options, DB};
use std::error::Error;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

pub mod error;
pub mod item;
pub mod managers;

#[derive(Clone)]
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

		Ok(Self { db_url: db_url.to_string() })
	}
}

#[tonic::async_trait]
impl LinearCombiner for LinearCombinerService {
	type GetNewDataStream = ReceiverStream<Result<LtObject, Status>>;
	type GetHistoricDataStream = ReceiverStream<Result<LtObject, Status>>;
	type GetDidMappingStream = ReceiverStream<Result<Mapping, Status>>;

	async fn sync_transformer(
		&self, request: Request<Streaming<TermObject>>,
	) -> Result<Response<Void>, Status> {
		let db = DB::open_cf(
			&Options::default(),
			&self.db_url,
			vec!["checkpoint", "index", "item", "mapping", "update"],
		)
		.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mut offset = CheckpointManager::read_checkpoint(&db).map_err(|e| e.into_status())?;

		let mut terms = Vec::new();
		let mut stream = request.into_inner();
		while let Some(term) = stream.message().await? {
			terms.push(term);
		}

		for term in terms {
			let domain = term.domain.to_be_bytes();
			let form = term.form.to_be_bytes();

			let (x, is_x_new) = IndexManager::get_index(&db, term.from.clone(), offset)
				.map_err(|e| e.into_status())?;

			// If x is new, write new mapping and increment the offset
			if is_x_new {
				MappingManager::write_mapping(&db, x.to_vec(), term.from.clone())
					.map_err(|e| e.into_status())?;
				offset += 1;
			}
			let (y, is_y_new) = IndexManager::get_index(&db, term.to.clone(), offset)
				.map_err(|e| e.into_status())?;

			// If y is new, write new mapping and increment the offset
			if is_y_new {
				MappingManager::write_mapping(&db, y.to_vec(), term.to.clone())
					.map_err(|e| e.into_status())?;
				offset += 1;
			}

			let mut key = Vec::new();
			key.extend_from_slice(&domain);
			key.extend_from_slice(&form);
			key.extend_from_slice(&x);
			key.extend_from_slice(&y);

			println!(
				"Received Item({}, {}, {})",
				u32::from_be_bytes(x),
				u32::from_be_bytes(y),
				term.weight
			);

			let value = ItemManager::update_value(&db, key.clone(), term.weight, term.timestamp)
				.map_err(|e| e.into_status())?;
			UpdateManager::set_value(&db, key.clone(), value, term.timestamp)
				.map_err(|e| e.into_status())?;
		}

		CheckpointManager::write_checkpoint(&db, offset).map_err(|e| e.into_status())?;

		Ok(Response::new(Void {}))
	}

	async fn get_did_mapping(
		&self, request: Request<MappingQuery>,
	) -> Result<Response<Self::GetDidMappingStream>, Status> {
		let mapping_query = request.into_inner();
		let db = DB::open_cf(&Options::default(), &self.db_url, vec!["mapping"])
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mappings = MappingManager::read_mappings(&db, mapping_query.start, mapping_query.size)
			.map_err(|e| e.into_status())?;

		let (tx, rx) = channel(1);
		for x in mappings.clone() {
			let x_obj: Mapping = x.into();
			if let Err(e) = tx.send(Ok(x_obj)).await {
				e.0?;
			}
		}
		Ok(Response::new(ReceiverStream::new(rx)))
	}

	async fn get_new_data(
		&self, request: Request<LtBatch>,
	) -> Result<Response<Self::GetNewDataStream>, Status> {
		let batch = request.into_inner();
		let db = DB::open_cf(&Options::default(), &self.db_url, vec!["update"])
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mut prefix = Vec::new();
		prefix.extend_from_slice(&batch.domain.to_be_bytes());
		prefix.extend_from_slice(&batch.form.to_be_bytes());
		let items = UpdateManager::read_batch(&db, prefix.clone(), batch.size)
			.map_err(|e| e.into_status())?;

		let (tx, rx) = channel(1);
		for x in items.clone() {
			let x_obj: LtObject = x.into();
			if let Err(e) = tx.send(Ok(x_obj)).await {
				e.0?;
			}
		}

		UpdateManager::delete_batch(&db, prefix, items).map_err(|e| e.into_status())?;

		Ok(Response::new(ReceiverStream::new(rx)))
	}

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

		let items = ItemManager::read_window(&db, prefix, (x_start, y_start), (x_end, y_end))
			.map_err(|e| e.into_status())?;

		println!("Read items: {:?}", items);

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = "[::1]:50052".parse()?;
	let service = LinearCombinerService::new("lc-storage")?;
	Server::builder().add_service(LinearCombinerServer::new(service)).serve(addr).await?;
	Ok(())
}
