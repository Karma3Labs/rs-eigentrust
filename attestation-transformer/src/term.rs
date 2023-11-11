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
}

pub trait Validation {
	fn validate(&self) -> (PublicKey, bool);
}

pub trait IntoTerm {
	const WEIGHT: u32;
	const DOMAIN: u32;

	fn into_term(self) -> Term;
}
