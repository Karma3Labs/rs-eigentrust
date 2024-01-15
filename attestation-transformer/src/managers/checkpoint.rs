use crate::error::AttTrError;
use rocksdb::{Options, DB};

#[derive(Debug)]
pub struct CheckpointManager {
	db: DB,
}

impl CheckpointManager {
	pub fn new(db: &str) -> Result<Self, AttTrError> {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		let db = DB::open_cf(&opts, db, vec!["checkpoint"]).map_err(|e| AttTrError::DbError(e))?;
		let checkpoint = db.get(b"event_count").map_err(|x| AttTrError::DbError(x))?;
		if let None = checkpoint {
			let zero = 0u32.to_be_bytes();
			db.put(b"event_count", zero).map_err(|e| AttTrError::DbError(e))?;
			db.put(b"term_count", zero).map_err(|e| AttTrError::DbError(e))?;
		}
		Ok(Self { db })
	}

	pub fn read_checkpoint(&self) -> Result<(u32, u32), AttTrError> {
		let event_offset_bytes_opt =
			self.db.get(b"event_count").map_err(|e| AttTrError::DbError(e))?;
		let term_offset_bytes_opt =
			self.db.get(b"term_count").map_err(|e| AttTrError::DbError(e))?;

		let checkpoint_offset_bytes = event_offset_bytes_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		let count_offset_bytes = term_offset_bytes_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});

		let checkpoint = u32::from_be_bytes(checkpoint_offset_bytes);
		let count = u32::from_be_bytes(count_offset_bytes);
		Ok((checkpoint, count))
	}

	pub fn write_checkpoint(&self, checkpoint: u32, count: u32) -> Result<(), AttTrError> {
		self.db
			.put(b"event_count", checkpoint.to_be_bytes())
			.map_err(|e| AttTrError::DbError(e))?;
		self.db.put(b"term_count", count.to_be_bytes()).map_err(|e| AttTrError::DbError(e))?;
		Ok(())
	}

	pub fn drop(self) -> Result<(), AttTrError> {
		self.db.drop_cf("checkpoint").map_err(|e| AttTrError::DbError(e))?;
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use crate::managers::checkpoint::CheckpointManager;

	#[test]
	fn should_write_read_checkpoint() {
		let checkpoint_manager = CheckpointManager::new("att-tr-checkpoint-test-storage").unwrap();
		checkpoint_manager.write_checkpoint(15, 14).unwrap();
		let (checkpoint, count) = checkpoint_manager.read_checkpoint().unwrap();
		assert_eq!(checkpoint, 15);
		assert_eq!(count, 14);
	}
}
