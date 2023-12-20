use serde_derive::{Deserialize, Serialize};

use crate::error::AttTrError;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum Schema {
	PkhEth,
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
		// 4 parts: did, pkh, eth, [public key hash]
		if part_slices.len() != 4 {
			return Err(AttTrError::ParseError);
		}
		let prefix = part_slices[0];
		if prefix != "did" {
			return Err(AttTrError::ParseError);
		}
		let schema = match part_slices[1..3] {
			["pkh", "eth"] => Schema::PkhEth,
			_ => return Err(AttTrError::ParseError),
		};
		let key = hex::decode(part_slices[3]).map_err(|_| AttTrError::ParseError)?;

		Ok(Self { schema, key })
	}
}

impl Into<String> for Did {
	fn into(self) -> String {
		let schema = match self.schema {
			Schema::PkhEth => "pkh:eth",
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
		let did_string = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string();
		let did = Did::parse(did_string.clone()).unwrap();
		assert_eq!(did.schema, Schema::PkhEth);
		assert_eq!(
			did.key,
			hex::decode("90f8bf6a479f320ead074411a4b0e7944ea8c9c2").unwrap()
		);

		let did_new_string: String = did.into();

		assert_eq!(did_string, did_new_string);
	}
}
