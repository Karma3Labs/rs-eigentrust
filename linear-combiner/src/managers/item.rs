use rocksdb::DB;

use crate::{error::LcError, item::LtItem};

#[derive(Debug)]
pub struct ItemManager;

impl ItemManager {
	pub fn get_value(db: &DB, key: &Vec<u8>) -> Result<f32, LcError> {
		let cf = db.cf_handle("item").ok_or_else(|| LcError::NotFoundError)?;
		let value_opt = db.get_cf(&cf, &key).map_err(|e| LcError::DbError(e))?;
		let value_bytes = value_opt.map_or([0; 4], |x| {
			let mut bytes: [u8; 4] = [0; 4];
			bytes.copy_from_slice(&x);
			bytes
		});
		Ok(f32::from_be_bytes(value_bytes))
	}

	pub fn update_value(db: &DB, key: Vec<u8>, weight: f32) -> Result<f32, LcError> {
		let cf = db.cf_handle("item").ok_or_else(|| LcError::NotFoundError)?;
		let value = Self::get_value(db, &key)?;
		let new_value = value + weight;
		db.put_cf(&cf, key.clone(), new_value.to_be_bytes()).map_err(|e| LcError::DbError(e))?;
		Ok(new_value)
	}

	pub fn read_window(
		db: &DB, prefix: Vec<u8>, p0: (u32, u32), p1: (u32, u32),
	) -> Result<Vec<LtItem>, LcError> {
		let cf = db.cf_handle("item").ok_or_else(|| LcError::NotFoundError)?;
		let mut items = Vec::new();
		(p0.0..=p1.0).zip(p0.1..=p1.1).into_iter().for_each(|(x, y)| {
			let mut key = Vec::new();
			key.extend_from_slice(&prefix);
			key.extend_from_slice(&x.to_be_bytes());
			key.extend_from_slice(&y.to_be_bytes());

			let item_res = db.get_cf(&cf, key.clone());
			if let Ok(Some(value)) = item_res {
				let let_item = LtItem::from_raw(key, value);
				items.push(let_item);
			}
		});
		Ok(items)
	}
}

#[cfg(test)]
mod test {
	use crate::{item::LtItem, managers::item::ItemManager};
	use rocksdb::{Options, DB};

	#[test]
	fn should_update_item() {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, "lc-ui-test-storage", vec!["item"]).unwrap();

		let key = vec![0; 8];
		let weight = 50.;

		let new_value = ItemManager::update_value(&db, key.clone(), weight).unwrap();
		let value = ItemManager::get_value(&db, &key).unwrap();

		assert_eq!(value, new_value);
	}

	#[test]
	fn should_read_window() {
		let mut opts = Options::default();
		opts.create_missing_column_families(true);
		opts.create_if_missing(true);
		let db = DB::open_cf(&opts, "lc-rw-test-storage", vec!["item"]).unwrap();

		let prefix = vec![0; 8];

		let x1: u32 = 0;
		let y1: u32 = 0;

		let x2: u32 = 1;
		let y2: u32 = 1;

		let weight = 50.;

		let mut key1 = Vec::new();
		key1.extend_from_slice(&prefix);
		key1.extend_from_slice(&x1.to_be_bytes());
		key1.extend_from_slice(&y1.to_be_bytes());

		let mut key2 = Vec::new();
		key2.extend_from_slice(&prefix);
		key2.extend_from_slice(&x2.to_be_bytes());
		key2.extend_from_slice(&y2.to_be_bytes());

		let prev_value1 = ItemManager::get_value(&db, &key1).unwrap();
		let prev_value2 = ItemManager::get_value(&db, &key2).unwrap();
		ItemManager::update_value(&db, key1.clone(), weight).unwrap();
		ItemManager::update_value(&db, key2.clone(), weight).unwrap();
		let new_item1 = LtItem::new(x1, y1, prev_value1 + weight);
		let new_item2 = LtItem::new(x2, y2, prev_value2 + weight);
		let new_items = vec![new_item1, new_item2];

		let items = ItemManager::read_window(&db, prefix, (x1, y1), (x2, y2)).unwrap();

		assert_eq!(new_items, items);
	}
}
