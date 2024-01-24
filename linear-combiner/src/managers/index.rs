use crate::error::LcError;
use rocksdb::DB;

#[derive(Debug)]
pub struct IndexManager;

impl IndexManager {
	pub fn get_index(db: &DB, source: String, offset: u32) -> Result<([u8; 4], bool), LcError> {
		let cf = db.cf_handle("index").ok_or_else(|| LcError::NotFoundError)?;

		let key = source.as_bytes();
		let source_index = db.get_cf(&cf, key).map_err(|e| LcError::DbError(e))?;

		let x = if let Some(from_i) = source_index {
			let from_bytes: [u8; 4] = from_i.try_into().map_err(|_| LcError::ParseError)?;
			(from_bytes, false)
		} else {
			let curr_offset = offset.to_be_bytes();
			db.put_cf(&cf, key, curr_offset).map_err(|e| LcError::DbError(e))?;
			(curr_offset, true)
		};

		Ok(x)
	}
}

#[cfg(test)]
mod test {
	use crate::managers::index::IndexManager;
	use rocksdb::{Options, DB};

	#[test]
	fn should_update_and_get_index() {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, "lc-ugi-test-storage", vec!["index"]).unwrap();

		let source = "90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string();
		let offset = 15;

		let (index, is_new) = IndexManager::get_index(&db, source.clone(), offset).unwrap();

		let mut bytes = [0; 4];
		bytes.copy_from_slice(&index);
		let i = u32::from_be_bytes(bytes);

		assert_eq!(i, 15);
		assert!(is_new);
	}
}
