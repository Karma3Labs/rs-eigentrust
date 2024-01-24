use proto_buf::combiner::{LtObject, Mapping};

#[derive(Debug, Clone, PartialEq)]
pub struct LtItem {
	x: u32,
	y: u32,
	value: f32,
}

impl LtItem {
	pub fn new(x: u32, y: u32, value: f32) -> Self {
		LtItem { x, y, value }
	}

	pub fn key_bytes(&self) -> Vec<u8> {
		let x_bytes = self.x.to_be_bytes();
		let y_bytes = self.y.to_be_bytes();

		let mut bytes = Vec::new();
		bytes.extend_from_slice(&x_bytes);
		bytes.extend_from_slice(&y_bytes);

		bytes
	}

	pub fn from_raw<I: AsRef<[u8]>>(key: I, value: I) -> Self {
		let mut key_bytes = [0; 16];
		key_bytes.copy_from_slice(key.as_ref());

		let mut value_bytes = [0; 4];
		value_bytes.copy_from_slice(value.as_ref());

		let mut x_bytes = [0; 4];
		let mut y_bytes = [0; 4];
		x_bytes.copy_from_slice(&key_bytes[8..12]);
		y_bytes.copy_from_slice(&key_bytes[12..]);

		let x = u32::from_be_bytes(x_bytes);
		let y = u32::from_be_bytes(y_bytes);
		let value = f32::from_be_bytes(value_bytes);

		Self { x, y, value }
	}
}

impl Into<LtObject> for LtItem {
	fn into(self) -> LtObject {
		LtObject { x: self.x, y: self.y, value: self.value }
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingItem {
	id: u32,
	did: String,
}

impl MappingItem {
	pub fn new(id: u32, did: String) -> Self {
		Self { id, did }
	}

	pub fn from_raw<I: AsRef<[u8]>>(id: I, did: I) -> Self {
		let mut id_bytes = [0; 4];
		id_bytes.copy_from_slice(id.as_ref());

		let id = u32::from_be_bytes(id_bytes);
		let did = hex::encode(did);

		Self { id, did }
	}
}

impl Into<Mapping> for MappingItem {
	fn into(self) -> Mapping {
		Mapping { id: self.id, did: self.did }
	}
}
