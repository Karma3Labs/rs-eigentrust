use crate::{error::LcError, item::MappingItem};
use rocksdb::{Direction, IteratorMode, DB};

#[derive(Debug)]
pub struct MappingManager;

impl MappingManager {
	pub fn write_mapping(db: &DB, index: Vec<u8>, key: String) -> Result<(), LcError> {
		let cf = db.cf_handle("mapping").ok_or_else(|| LcError::NotFoundError)?;
		db.put_cf(&cf, index, key.as_bytes()).map_err(|e| LcError::DbError(e))?;
		Ok(())
	}

	pub fn read_mappings(db: &DB, start: u32, n: u32) -> Result<Vec<MappingItem>, LcError> {
		let cf = db.cf_handle("mapping").ok_or_else(|| LcError::NotFoundError)?;
		let iter = db.iterator_cf(
			&cf,
			IteratorMode::From(&start.to_be_bytes(), Direction::Forward),
		);

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
}
