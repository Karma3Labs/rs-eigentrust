use rocksdb::DB;

use crate::{error::LcError, item::LtItem};

#[derive(Debug)]
pub struct ItemManager;

impl ItemManager {
	pub fn get_value(db: &DB, key: &Vec<u8>) -> Result<LtItem, LcError> {
		let cf = db.cf_handle("item").ok_or(LcError::NotFoundError)?;
		let value_opt = db.get_cf(&cf, key).map_err(LcError::DbError)?;
		let item = value_opt.map_or(LtItem::default(), |value| LtItem::from_raw(key, &value));
		Ok(item)
	}

	pub fn update_value(
		db: &DB, key: Vec<u8>, weight: f32, timestamp: u64,
	) -> Result<f32, LcError> {
		let cf = db.cf_handle("item").ok_or(LcError::NotFoundError)?;
		let item = Self::get_value(db, &key)?;

		let new_value = item.value + weight;

		let mut bytes = Vec::new();
		bytes.extend_from_slice(&new_value.to_be_bytes());
		bytes.extend_from_slice(&timestamp.to_be_bytes());

		db.put_cf(&cf, key.clone(), bytes).map_err(LcError::DbError)?;
		Ok(new_value)
	}

	pub fn read_window(
		db: &DB, prefix: Vec<u8>, p0: (u32, u32), p1: (u32, u32),
	) -> Result<Vec<LtItem>, LcError> {
		let cf = db.cf_handle("item").ok_or(LcError::NotFoundError)?;
		let mut items = Vec::new();
		(p0.0..=p1.0).for_each(|x| {
			(p0.1..=p1.1).for_each(|y| {
				let mut key = Vec::new();
				key.extend_from_slice(&prefix);
				key.extend_from_slice(&x.to_be_bytes());
				key.extend_from_slice(&y.to_be_bytes());

				println!("Looking for {} {}", x, y);

				let item_res = db.get_cf(&cf, key.clone());
				if let Ok(Some(value)) = item_res {
					let let_item = LtItem::from_raw(key, value);
					items.push(let_item);
				}
			});
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

		let key = vec![0; 16];
		let weight = 50.;
		let timestamp = 0;

		let new_value = ItemManager::update_value(&db, key.clone(), weight, timestamp).unwrap();
		let item = ItemManager::get_value(&db, &key).unwrap();

		assert_eq!(item.value, new_value);
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

		let timestamp = 0;

		let mut key1 = Vec::new();
		key1.extend_from_slice(&prefix);
		key1.extend_from_slice(&x1.to_be_bytes());
		key1.extend_from_slice(&y1.to_be_bytes());

		let mut key2 = Vec::new();
		key2.extend_from_slice(&prefix);
		key2.extend_from_slice(&x2.to_be_bytes());
		key2.extend_from_slice(&y2.to_be_bytes());

		let prev_item1 = ItemManager::get_value(&db, &key1).unwrap();
		let prev_item2 = ItemManager::get_value(&db, &key2).unwrap();
		ItemManager::update_value(&db, key1.clone(), weight, timestamp).unwrap();
		ItemManager::update_value(&db, key2.clone(), weight, timestamp).unwrap();
		let new_item1 = LtItem::new(x1, y1, prev_item1.value + weight, timestamp);
		let new_item2 = LtItem::new(x2, y2, prev_item2.value + weight, timestamp);
		let new_items = vec![new_item1, new_item2];

		let items = ItemManager::read_window(&db, prefix, (x1, y1), (x2, y2)).unwrap();

		assert_eq!(new_items, items);
	}
}
