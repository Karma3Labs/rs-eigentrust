use serde_derive::{Deserialize, Serialize};

use crate::error::AttTrError;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum Schema {
	PkhEth,
	Snap,
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

	pub fn parse_pkh_eth(value: String) -> Result<Self, AttTrError> {
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

		let addr = part_slices[3].trim_start_matches("0x");
		let key = hex::decode(addr).map_err(|_| AttTrError::ParseError)?;

		Ok(Self { schema, key })
	}

	pub fn parse_snap(value: String) -> Result<Self, AttTrError> {
		let parts = value.split("://");
		let part_slices: Vec<&str> = parts.into_iter().collect();
		// 4 parts: did, pkh, eth, [public key hash]
		if part_slices.len() != 2 {
			return Err(AttTrError::ParseError);
		}

		let prefix = part_slices[0];
		if prefix != "snap" {
			return Err(AttTrError::ParseError);
		}

		let addr = part_slices[1].trim_start_matches("0x");
		let key = hex::decode(addr).map_err(|_| AttTrError::ParseError)?;

		Ok(Self { schema: Schema::Snap, key })
	}
}

impl Into<String> for Did {
	fn into(self) -> String {
		let key = hex::encode(self.key);
		match self.schema {
			Schema::PkhEth => format!("did:pkh:eth:{}", key),
			Schema::Snap => format!("snap://{}", key),
		}
	}
}

#[cfg(test)]
mod test {
	use crate::did::Schema;

	use super::Did;

	#[test]
	fn test_did_parsing() {
		let did_string = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_string();
		let did = Did::parse_pkh_eth(did_string.clone()).unwrap();
		assert_eq!(did.schema, Schema::PkhEth);
		assert_eq!(
			did.key,
			hex::decode("90f8bf6a479f320ead074411a4b0e7944ea8c9c2").unwrap()
		);

		let did_new_string: String = did.into();

		assert_eq!(did_string, did_new_string);
	}
}
