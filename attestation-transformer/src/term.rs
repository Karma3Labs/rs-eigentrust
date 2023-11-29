use proto_buf::transformer::{Form, TermObject};
use secp256k1::PublicKey;

use crate::error::AttTrError;

enum TermForm {
	Trust,
	Distrust,
}

impl From<u8> for TermForm {
	fn from(value: u8) -> Self {
		match value {
			0 => Self::Trust,
			1 => Self::Distrust,
			_ => panic!("Invalid Term form"),
		}
	}
}

impl Into<u8> for TermForm {
	fn into(self) -> u8 {
		match self {
			Self::Trust => 0,
			Self::Distrust => 1,
		}
	}
}

impl Into<Form> for TermForm {
	fn into(self) -> Form {
		match self {
			Self::Trust => Form::Trust,
			Self::Distrust => Form::Distrust,
		}
	}
}

pub struct Term {
	from: String,
	to: String,
	weight: u32,
	domain: u32,
	form: TermForm,
}

impl Term {
	pub fn new(from: String, to: String, weight: u32, domain: u32, is_trust: bool) -> Term {
		Term {
			from,
			to,
			weight,
			domain,
			form: if is_trust { TermForm::Trust } else { TermForm::Distrust },
		}
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

	pub fn from_bytes(mut bytes: Vec<u8>) -> Result<Self, AttTrError> {
		let from_bytes: Vec<u8> = bytes.drain(..20).collect();
		let to_bytes: Vec<u8> = bytes.drain(..20).collect();
		let weight_bytes: [u8; 4] = bytes
			.drain(..4)
			.collect::<Vec<u8>>()
			.try_into()
			.map_err(|_| AttTrError::SerialisationError)?;
		let domain_bytes: [u8; 4] = bytes
			.drain(..4)
			.collect::<Vec<u8>>()
			.try_into()
			.map_err(|_| AttTrError::SerialisationError)?;
		let form_byte = bytes[0];

		let from = String::from_utf8(from_bytes).map_err(|_| AttTrError::SerialisationError)?;
		let to = String::from_utf8(to_bytes).map_err(|_| AttTrError::SerialisationError)?;
		let weight = u32::from_be_bytes(weight_bytes);
		let domain = u32::from_be_bytes(domain_bytes);
		let form = TermForm::from(form_byte);

		Ok(Self { from, to, weight, domain, form })
	}
}

impl Into<TermObject> for Term {
	fn into(self) -> TermObject {
		let form: Form = self.form.into();
		TermObject {
			from: self.from,
			to: self.to,
			weight: self.weight,
			domain: self.domain,
			form: form.into(),
		}
	}
}

pub trait Validation {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError>;
}

pub trait IntoTerm: Validation {
	const DOMAIN: u32;

	fn into_term(self) -> Result<Term, AttTrError>;
}
