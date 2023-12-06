use serde_derive::{Deserialize, Serialize};

use crate::error::AttTrError;

#[derive(Deserialize, Serialize, Clone)]
pub enum Schema {
	Pkh,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Did {
	schema: Schema,
	pub key: Vec<u8>,
}

impl Did {
	pub fn new(schema: Schema, key: Vec<u8>) -> Self {
		Self { schema, key }
	}

	pub fn parse(value: String) -> Result<Self, AttTrError> {
		let parts = value.split(":");
		let part_slices: Vec<&str> = parts.into_iter().collect();
		// 3 parts: did, pkh, [public key hash]
		if part_slices.len() != 3 {
			return Err(AttTrError::ParseError);
		}
		let prefix = part_slices[0];
		if prefix != "did" {
			return Err(AttTrError::ParseError);
		}
		let schema = match part_slices[1] {
			"pkh" => Schema::Pkh,
			_ => return Err(AttTrError::ParseError),
		};
		let key = hex::decode(part_slices[2]).map_err(|_| AttTrError::ParseError)?;

		Ok(Self { schema, key })
	}
}

impl Into<String> for Did {
	fn into(self) -> String {
		let schema = match self.schema {
			Schema::Pkh => "pkh",
		};
		let pkh = hex::encode(self.key);
		let did_string = format!("did:{}:{}", schema, pkh);

		did_string
	}
}
