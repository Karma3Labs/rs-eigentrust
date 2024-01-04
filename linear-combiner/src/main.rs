use error::LcError;
use item::{LtItem, MappingItem};
use proto_buf::{
	combiner::{
		linear_combiner_server::{LinearCombiner, LinearCombinerServer},
		LtBatch, LtHistoryBatch, LtObject, Mapping, MappingQuery,
	},
	common::Void,
	transformer::TermObject,
};
use rocksdb::{Direction, DB};
use rocksdb::{IteratorMode, WriteBatch};
use std::error::Error;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

mod error;
mod item;

#[derive(Clone)]
struct LinearCombinerService {
	main_db: String,
	updates_db: String,
	mapping_db: String,
}

impl LinearCombinerService {
	pub fn new(main_db_url: &str, updates_db_url: &str, mapping_db: &str) -> Result<Self, LcError> {
		let main_db = DB::open_default(main_db_url).map_err(|x| LcError::DbError(x))?;
		let checkpoint = main_db.get(b"checkpoint").map_err(|x| LcError::DbError(x))?;
		if let None = checkpoint {
			let count = 0u32.to_be_bytes();
			main_db.put(b"checkpoint", count).map_err(|x| LcError::DbError(x))?;
		}

		Ok(Self {
			main_db: main_db_url.to_string(),
			updates_db: updates_db_url.to_string(),
			mapping_db: mapping_db.to_string(),
		})
	}

	fn read_checkpoint(db: &DB) -> Result<u32, LcError> {
		let offset_bytes_opt = db.get(b"checkpoint").map_err(|x| LcError::DbError(x))?;
		let offset_bytes = offset_bytes_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		let offset = u32::from_be_bytes(offset_bytes);
		Ok(offset)
	}

	fn write_checkpoint(db: &DB, count: u32) -> Result<(), LcError> {
		db.put(b"checkpoint", count.to_be_bytes()).map_err(|x| LcError::DbError(x))?;
		Ok(())
	}

	fn get_index(
		db: &DB, mapping_db: &DB, source: String, offset: &mut u32,
	) -> Result<[u8; 4], LcError> {
		let key = hex::decode(source).map_err(|_| LcError::ParseError)?;
		let source_index = db.get(&key).map_err(|e| LcError::DbError(e))?;

		let x = if let Some(from_i) = source_index {
			let from_bytes: [u8; 4] = from_i.try_into().map_err(|_| LcError::ParseError)?;
			from_bytes
		} else {
			let curr_offset = offset.to_be_bytes();
			db.put(&key, curr_offset).map_err(|e| LcError::DbError(e))?;
			mapping_db.put(curr_offset, key).map_err(|e| LcError::DbError(e))?;
			*offset += 1;
			curr_offset
		};

		Ok(x)
	}

	fn read_mappings(mapping_db: &DB, start: u32, n: u32) -> Result<Vec<MappingItem>, LcError> {
		let iter =
			mapping_db.iterator(IteratorMode::From(&start.to_be_bytes(), Direction::Forward));

		let size = usize::try_from(n).map_err(|_| LcError::ParseError)?;
		let mappings = iter.take(size).try_fold(Vec::new(), |mut acc, item| {
			item.map(|(key, value)| {
				let mapping = MappingItem::from_raw(key, value);
				acc.push(mapping);
				acc
			})
			.map_err(|e| LcError::DbError(e))
		});
		mappings
	}

	fn get_value(main_db: &DB, key: &Vec<u8>) -> Result<u32, LcError> {
		let value_opt = main_db.get(&key).map_err(|e| LcError::DbError(e))?;
		let value_bytes = value_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		Ok(u32::from_be_bytes(value_bytes))
	}

	fn update_value(
		main_db: &DB, updates_db: &DB, key: Vec<u8>, weight: u32,
	) -> Result<(), LcError> {
		let value = Self::get_value(main_db, &key)?;
		let new_value = (value + weight).to_be_bytes();
		main_db.put(key.clone(), new_value).map_err(|e| LcError::DbError(e))?;
		updates_db.put(key.clone(), new_value).map_err(|e| LcError::DbError(e))?;
		Ok(())
	}

	fn read_batch(updates_db: &DB, prefix: Vec<u8>, n: u32) -> Result<Vec<LtItem>, LcError> {
		let mut iter = updates_db.prefix_iterator(prefix);
		iter.set_mode(IteratorMode::Start);

		let size = usize::try_from(n).map_err(|_| LcError::ParseError)?;
		let items = iter.take(size).try_fold(Vec::new(), |mut acc, item| {
			item.map(|(key, value)| {
				let lt_item = LtItem::from_raw(key, value);
				acc.push(lt_item);
				acc
			})
			.map_err(|e| LcError::DbError(e))
		});

		items
	}

	fn delete_batch(updates_db: &DB, prefix: Vec<u8>, items: Vec<LtItem>) -> Result<(), LcError> {
		let mut batch = WriteBatch::default();
		items.iter().for_each(|x| {
			let mut key = Vec::new();
			key.extend_from_slice(&prefix);
			key.extend_from_slice(&x.key_bytes());
			batch.delete(key);
		});
		updates_db.write(batch).map_err(|e| LcError::DbError(e))?;
		Ok(())
	}

	fn read_window(main_db: &DB, prefix: Vec<u8>, p0: (u32, u32), p1: (u32, u32)) -> Vec<LtItem> {
		let mut items = Vec::new();
		(p0.0..=p1.0).zip(p0.1..=p1.1).into_iter().for_each(|(x, y)| {
			let mut key = Vec::new();
			key.extend_from_slice(&prefix);
			key.extend_from_slice(&x.to_be_bytes());
			key.extend_from_slice(&y.to_be_bytes());

			let item_res = main_db.get(key.clone());
			if let Ok(Some(value)) = item_res {
				let let_item = LtItem::from_raw(key, value);
				items.push(let_item);
			}
		});
		items
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
		let main_db = DB::open_default(&self.main_db)
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;
		let updates_db = DB::open_default(&self.updates_db)
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;
		let mapping_db = DB::open_default(&self.mapping_db)
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mut offset = Self::read_checkpoint(&main_db).map_err(|e| e.into_status())?;

		let mut terms = Vec::new();
		let mut stream = request.into_inner();
		while let Some(term) = stream.message().await? {
			terms.push(term);
		}

		for term in terms {
			let domain = term.domain.to_be_bytes();
			let form = term.form.to_be_bytes();

			let x = Self::get_index(&main_db, &mapping_db, term.from.clone(), &mut offset)
				.map_err(|e| e.into_status())?;
			let y = Self::get_index(&main_db, &mapping_db, term.to.clone(), &mut offset)
				.map_err(|e| e.into_status())?;

			let mut key = Vec::new();
			key.extend_from_slice(&domain);
			key.extend_from_slice(&form);
			key.extend_from_slice(&x);
			key.extend_from_slice(&y);

			Self::update_value(&main_db, &updates_db, key.clone(), term.weight)
				.map_err(|e| e.into_status())?;
		}

		Self::write_checkpoint(&main_db, offset).map_err(|e| e.into_status())?;

		Ok(Response::new(Void {}))
	}

	async fn get_did_mapping(
		&self, request: Request<MappingQuery>,
	) -> Result<Response<Self::GetDidMappingStream>, Status> {
		let mapping_query = request.into_inner();
		let mapping_db = DB::open_default(&self.mapping_db)
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mappings = Self::read_mappings(&mapping_db, mapping_query.start, mapping_query.size)
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
		let updates_db = DB::open_default(&self.updates_db)
			.map_err(|e| Status::internal(format!("Internal error: {}", e)))?;

		let mut prefix = Vec::new();
		prefix.extend_from_slice(&batch.domain.to_be_bytes());
		prefix.extend_from_slice(&batch.form.to_be_bytes());
		let items = Self::read_batch(&updates_db, prefix.clone(), batch.size)
			.map_err(|e| e.into_status())?;

		let (tx, rx) = channel(1);
		for x in items.clone() {
			let x_obj: LtObject = x.into();
			if let Err(e) = tx.send(Ok(x_obj)).await {
				e.0?;
			}
		}

		Self::delete_batch(&updates_db, prefix, items).map_err(|e| e.into_status())?;

		Ok(Response::new(ReceiverStream::new(rx)))
	}

	async fn get_historic_data(
		&self, request: Request<LtHistoryBatch>,
	) -> Result<Response<Self::GetHistoricDataStream>, Status> {
		let batch = request.into_inner();
		let main_db = DB::open_default(&self.main_db)
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

		let items = Self::read_window(&main_db, prefix, (x_start, y_start), (x_end, y_end));

		let (tx, rx) = channel(1);
		for x in items.clone() {
			let x_obj: LtObject = x.into();
			if let Err(e) = tx.send(Ok(x_obj)).await {
				e.0?;
			}
		}

		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = "[::1]:50052".parse()?;
	let service =
		LinearCombinerService::new("lc-storage", "lc-updates-storage", "lc-mapping-storage")?;
	Server::builder().add_service(LinearCombinerServer::new(service)).serve(addr).await?;
	Ok(())
}

#[cfg(test)]
mod test {
	use rocksdb::DB;

	use crate::{item::LtItem, LinearCombinerService};
	#[test]
	fn should_write_read_checkpoint() {
		let db = DB::open_default("lc-checkpoint-test-storage").unwrap();
		LinearCombinerService::write_checkpoint(&db, 15).unwrap();
		let checkpoint = LinearCombinerService::read_checkpoint(&db).unwrap();
		assert_eq!(checkpoint, 15);
	}

	#[test]
	fn should_update_and_get_index() {
		let main_db = DB::open_default("lc-index-test-storage").unwrap();
		let mapping_db = DB::open_default("lc-mapping-test-storage").unwrap();
		let source = "90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string();
		let mut offset = 0;

		let index =
			LinearCombinerService::get_index(&main_db, &mapping_db, source, &mut offset).unwrap();

		let mut bytes = [0; 4];
		bytes.copy_from_slice(&index);
		let i = u32::from_be_bytes(bytes);

		assert_eq!(i, 0);
	}

	#[test]
	fn should_update_item() {
		let main_db = DB::open_default("lc-items-test-storage").unwrap();
		let updates_db = DB::open_default("lc-updates-test-storage").unwrap();
		let key = vec![0; 8];
		let weight = 50;

		let prev_value = LinearCombinerService::get_value(&main_db, &key).unwrap();
		LinearCombinerService::update_value(&main_db, &updates_db, key.clone(), weight).unwrap();
		let value = LinearCombinerService::get_value(&main_db, &key).unwrap();

		assert_eq!(value, prev_value + weight);
	}

	#[test]
	fn should_read_delete_batch() {
		let main_db = DB::open_default("lc-rd-items-test-storage").unwrap();
		let updates_db = DB::open_default("lc-rd-updates-test-storage").unwrap();
		let prefix = vec![0; 8];
		let key = vec![0; 16];
		let weight = 50u32;

		let prev_value = LinearCombinerService::get_value(&main_db, &key).unwrap();
		LinearCombinerService::update_value(&main_db, &updates_db, key.clone(), weight).unwrap();

		let org_items =
			vec![LtItem::from_raw(key.clone(), (weight + prev_value).to_be_bytes().to_vec())];
		let items = LinearCombinerService::read_batch(&updates_db, prefix.clone(), 1).unwrap();
		assert_eq!(items, org_items);

		LinearCombinerService::delete_batch(&updates_db, prefix.clone(), items).unwrap();
		let items = LinearCombinerService::read_batch(&updates_db, prefix, 1).unwrap();
		assert_eq!(items, Vec::new());
	}

	#[test]
	fn should_read_window() {
		let main_db = DB::open_default("lc-rdw-items-test-storage").unwrap();
		let updates_db = DB::open_default("lc-rdw-updates-test-storage").unwrap();
		let prefix = vec![0; 8];

		let x1: u32 = 0;
		let y1: u32 = 0;

		let x2: u32 = 1;
		let y2: u32 = 1;

		let weight = 50u32;

		let mut key1 = Vec::new();
		key1.extend_from_slice(&prefix);
		key1.extend_from_slice(&x1.to_be_bytes());
		key1.extend_from_slice(&y1.to_be_bytes());

		let mut key2 = Vec::new();
		key2.extend_from_slice(&prefix);
		key2.extend_from_slice(&x2.to_be_bytes());
		key2.extend_from_slice(&y2.to_be_bytes());

		let prev_value1 = LinearCombinerService::get_value(&main_db, &key1).unwrap();
		let prev_value2 = LinearCombinerService::get_value(&main_db, &key2).unwrap();
		LinearCombinerService::update_value(&main_db, &updates_db, key1.clone(), weight).unwrap();
		LinearCombinerService::update_value(&main_db, &updates_db, key2.clone(), weight).unwrap();
		let new_item1 = LtItem::new(x1, y1, prev_value1 + weight);
		let new_item2 = LtItem::new(x2, y2, prev_value2 + weight);
		let new_items = vec![new_item1, new_item2];

		let items = LinearCombinerService::read_window(&main_db, prefix, (x1, y1), (x2, y2));

		assert_eq!(new_items, items);
	}
}
