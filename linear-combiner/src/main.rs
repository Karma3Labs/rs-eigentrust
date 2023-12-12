use did::Did;
use error::LcError;
use proto_buf::{
	combiner::{
		linear_combiner_server::{LinearCombiner, LinearCombinerServer},
		LtBatch, LtObject,
	},
	common::Void,
	transformer::TermObject,
};
use rocksdb::DB;
use std::error::Error;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

mod did;
mod error;
mod item;

#[derive(Clone)]
struct LinearCombinerService {
	main_db: String,
	updates_db: String,
}

impl LinearCombinerService {
	pub fn new(main_db_url: &str, updates_db_url: &str) -> Result<Self, LcError> {
		let main_db = DB::open_default(main_db_url).map_err(|x| LcError::DbError(x))?;
		let checkpoint = main_db.get(b"checkpoint").map_err(|x| LcError::DbError(x))?;
		if let None = checkpoint {
			let count = 0u32.to_be_bytes();
			main_db.put(b"checkpoint", count).map_err(|x| LcError::DbError(x))?;
		}

		Ok(Self { main_db: main_db_url.to_string(), updates_db: updates_db_url.to_string() })
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

	fn get_index(db: &DB, source: String, offset: &mut u32) -> Vec<u8> {
		let source_did = Did::parse(source).unwrap();
		let source_index = db.get(&source_did.key).unwrap();

		let x = if let Some(from_i) = source_index {
			from_i
		} else {
			let curr_offset = offset.to_be_bytes();
			db.put(&source_did.key, curr_offset).unwrap();
			*offset += 1;
			curr_offset.to_vec()
		};

		x
	}

	fn update_value(main_db: &DB, updates_db: &DB, key: Vec<u8>, weight: u32) {
		let value_bytes = main_db.get(&key).unwrap().map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		let value = u32::from_be_bytes(value_bytes);
		let new_value = (value + weight).to_be_bytes();
		main_db.put(key.clone(), new_value).unwrap();
		updates_db.put(key, new_value).unwrap();
	}
}

#[tonic::async_trait]
impl LinearCombiner for LinearCombinerService {
	type SyncCoreComputerStream = ReceiverStream<Result<LtObject, Status>>;

	async fn sync_transformer(
		&self, request: Request<Streaming<TermObject>>,
	) -> Result<Response<Void>, Status> {
		let main_db = DB::open_default(&self.main_db).unwrap();
		let updates_db = DB::open_default(&self.updates_db).unwrap();

		let mut offset = Self::read_checkpoint(&main_db).unwrap();

		let mut terms = Vec::new();
		let mut stream = request.into_inner();
		while let Some(term) = stream.message().await? {
			terms.push(term);
		}

		for term in terms {
			let x = Self::get_index(&main_db, term.from.clone(), &mut offset);
			let y = Self::get_index(&main_db, term.to.clone(), &mut offset);

			let mut key = Vec::new();
			key.extend_from_slice(&x);
			key.extend_from_slice(&y);

			Self::update_value(&main_db, &updates_db, key, term.weight);
		}

		Self::write_checkpoint(&main_db, offset).unwrap();

		Ok(Response::new(Void {}))
	}

	async fn sync_core_computer(
		&self, request: Request<LtBatch>,
	) -> Result<Response<Self::SyncCoreComputerStream>, Status> {
		let _req_obj = request.into_inner();
		let num_buffers = 4;
		let (tx, rx) = channel(num_buffers);
		for _ in 0..num_buffers {
			tx.send(Ok(LtObject { x: 0, y: 0, value: 0 })).await.unwrap();
		}
		Ok(Response::new(ReceiverStream::new(rx)))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let addr = "[::1]:50052".parse()?;
	let service = LinearCombinerService::new("lc-storage", "lc-updates-storage")?;
	Server::builder().add_service(LinearCombinerServer::new(service)).serve(addr).await?;
	Ok(())
}
