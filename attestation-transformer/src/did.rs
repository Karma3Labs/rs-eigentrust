use serde_derive::{Deserialize, Serialize};

use crate::error::AttTrError;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod test {
	use crate::did::Schema;

	use super::Did;

	#[test]
	fn test_did_parsing() {
		let did_string = "did:pkh:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string();
		let did = Did::parse(did_string.clone()).unwrap();
		assert_eq!(did.schema, Schema::Pkh);
		assert_eq!(
			did.key,
			hex::decode("90f8bf6a479f320ead074411a4b0e7944ea8c9c2").unwrap()
		);

		let did_new_string: String = did.into();

		assert_eq!(did_string, did_new_string);
	}
}
