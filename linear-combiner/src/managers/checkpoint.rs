use rocksdb::DB;

use crate::error::LcError;

#[derive(Debug)]
pub struct CheckpointManager;

impl CheckpointManager {
	pub fn init(db: &DB) -> Result<(), LcError> {
		let cf = db.cf_handle("checkpoint").ok_or(LcError::NotFoundError)?;
		let checkpoint = db.get_cf(&cf, b"participant_count").map_err(LcError::DbError)?;
		if checkpoint.is_none() {
			let zero = 0u32.to_be_bytes();
			db.put_cf(&cf, b"participant_count", zero).map_err(LcError::DbError)?;
		}

		Ok(())
	}

	pub fn read_checkpoint(db: &DB) -> Result<u32, LcError> {
		let cf = db.cf_handle("checkpoint").ok_or(LcError::NotFoundError)?;
		let offset_bytes_opt = db.get_cf(&cf, b"participant_count").map_err(LcError::DbError)?;
		let offset_bytes = offset_bytes_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		let offset = u32::from_be_bytes(offset_bytes);
		Ok(offset)
	}

	pub fn write_checkpoint(db: &DB, count: u32) -> Result<(), LcError> {
		let cf = db.cf_handle("checkpoint").ok_or(LcError::NotFoundError)?;
		db.put_cf(&cf, b"participant_count", count.to_be_bytes()).map_err(LcError::DbError)?;
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use rocksdb::{Options, DB};

	use super::*;

	#[test]
	fn should_write_read_checkpoint() {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, "lc-rwc-test-storage", vec!["checkpoint"]).unwrap();

		CheckpointManager::write_checkpoint(&db, 15).unwrap();
		let checkpoint = CheckpointManager::read_checkpoint(&db).unwrap();
		assert_eq!(checkpoint, 15);
	}
}
