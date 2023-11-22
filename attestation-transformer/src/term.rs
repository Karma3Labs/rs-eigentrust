use proto_buf::transformer::TermObject;
use secp256k1::PublicKey;

pub struct Term {
	from: String,
	to: String,
	weight: u32,
	domain: u32,
}

impl Term {
	pub fn new(from: String, to: String, weight: u32, domain: u32) -> Term {
		Term { from, to, weight, domain }
	}

	pub fn into_bytes(self) -> Vec<u8> {
		let mut bytes = Vec::new();

		let from_bytes = self.from.as_bytes();
		let to_bytes = self.to.as_bytes();
		let weight_bytes = self.weight.to_be_bytes();
		let domain_bytes = self.domain.to_be_bytes();

		bytes.extend_from_slice(from_bytes);
		bytes.extend_from_slice(to_bytes);
		bytes.extend_from_slice(&weight_bytes);
		bytes.extend_from_slice(&domain_bytes);

		bytes
	}

	pub fn from_bytes(mut bytes: Vec<u8>) -> Self {
		let from_bytes: Vec<u8> = bytes.drain(..20).collect();
		let to_bytes: Vec<u8> = bytes.drain(..20).collect();
		let weight_bytes: [u8; 4] = bytes.drain(..4).collect::<Vec<u8>>().try_into().unwrap();
		let domain_bytes: [u8; 4] = bytes.drain(..4).collect::<Vec<u8>>().try_into().unwrap();

		let from = String::from_utf8(from_bytes).unwrap();
		let to = String::from_utf8(to_bytes).unwrap();
		let weight = u32::from_be_bytes(weight_bytes);
		let domain = u32::from_be_bytes(domain_bytes);

		Self { from, to, weight, domain }
	}
}

impl Into<TermObject> for Term {
	fn into(self) -> TermObject {
		TermObject { from: self.from, to: self.to, weight: self.weight, domain: self.domain }
	}
}

pub trait Validation {
	fn validate(&self) -> (PublicKey, bool);
}

pub trait IntoTerm {
	const WEIGHT: u32;
	const DOMAIN: u32;

	fn into_term(self) -> Term;
}
