use crate::error::AttTrError;
use rocksdb::DB;

#[derive(Debug)]
pub struct CheckpointManager;

impl CheckpointManager {
	pub fn init(db: &DB) -> Result<(), AttTrError> {
		let cf = db.cf_handle("checkpoint").ok_or_else(|| AttTrError::NotFoundError)?;
		let checkpoint = db.get_cf(&cf, b"event_count").map_err(AttTrError::DbError)?;
		if checkpoint.is_none() {
			let zero = 0u32.to_be_bytes();
			db.put_cf(&cf, "event_count", zero).map_err(AttTrError::DbError)?;
			db.put_cf(&cf, b"term_count", zero).map_err(AttTrError::DbError)?;
		}
		Ok(())
	}

	pub fn read_checkpoint(db: &DB) -> Result<(u32, u32), AttTrError> {
		let cf = db.cf_handle("checkpoint").ok_or_else(|| AttTrError::NotFoundError)?;

		let event_offset_bytes_opt =
			db.get_cf(&cf, b"event_count").map_err(AttTrError::DbError)?;
		let term_offset_bytes_opt =
			db.get_cf(&cf, b"term_count").map_err(AttTrError::DbError)?;

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

	pub fn write_checkpoint(db: &DB, checkpoint: u32, count: u32) -> Result<(), AttTrError> {
		let cf = db.cf_handle("checkpoint").ok_or_else(|| AttTrError::NotFoundError)?;
		db.put_cf(&cf, b"event_count", checkpoint.to_be_bytes())
			.map_err(AttTrError::DbError)?;
		db.put_cf(&cf, b"term_count", count.to_be_bytes()).map_err(AttTrError::DbError)?;
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use rocksdb::{Options, DB};

	use crate::managers::checkpoint::CheckpointManager;

	#[test]
	fn should_write_read_checkpoint() {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, "att-wrc-test-storage", vec!["checkpoint"]).unwrap();

		CheckpointManager::init(&db).unwrap();
		CheckpointManager::write_checkpoint(&db, 15, 14).unwrap();
		let (checkpoint, count) = CheckpointManager::read_checkpoint(&db).unwrap();
		assert_eq!(checkpoint, 15);
		assert_eq!(count, 14);
	}
}
