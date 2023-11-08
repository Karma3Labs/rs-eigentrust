use rocksdb::{IteratorMode, DB};

fn main() {
	let data = [(b"k1", b"v1111"), (b"k2", b"v2222"), (b"k3", b"v3333")];
	let db = DB::open_default("att-tr-storage").unwrap();

	for (key, value) in &data {
		assert!(db.put(key, value).is_ok());
	}

	let res = db.get(b"k2").unwrap();
	println!("{:?}", res);

	db.delete(b"k2").unwrap();

	let iter = db.iterator(IteratorMode::Start);

	for (idx, (db_key, db_value)) in iter.map(Result::unwrap).enumerate() {
		println!("{:?}", idx);
		println!("{:?}", db_key);
		println!("{:?}", db_value);
	}
}
