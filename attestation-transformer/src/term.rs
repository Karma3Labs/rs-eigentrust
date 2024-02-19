use crate::error::AttTrError;
use proto_buf::transformer::{Form, TermObject};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TermForm {
	Trust,
	Distrust,
}

impl From<u8> for TermForm {
	fn from(value: u8) -> Self {
		match value {
			1 => Self::Trust,
			0 => Self::Distrust,
			_ => panic!("Invalid Term form"),
		}
	}
}

impl From<TermForm> for u8 {
	fn from(value: TermForm) -> Self {
		match value {
			TermForm::Trust => 1,
			TermForm::Distrust => 0,
		}
	}
}

impl From<TermForm> for Form {
	fn from(value: TermForm) -> Self {
		match value {
			TermForm::Trust => Self::Trust,
			TermForm::Distrust => Self::Distrust,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Term {
	from: String,
	to: String,
	weight: f32,
	domain: u32,
	pub(crate) form: TermForm,
	timestamp: u64,
}

impl Term {
	pub fn new(
		from: String, to: String, weight: f32, domain: u32, form: TermForm, timestamp: u64,
	) -> Term {
		Term { from, to, weight, domain, form, timestamp }
	}

	pub fn into_bytes(self) -> Result<Vec<u8>, AttTrError> {
		serde_json::to_vec(&self).map_err(AttTrError::SerdeError)
	}

	pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, AttTrError> {
		serde_json::from_slice::<Self>(bytes.as_slice()).map_err(AttTrError::SerdeError)
	}
}

impl From<Term> for TermObject {
	fn from(value: Term) -> Self {
		let form: Form = value.form.into();
		Self {
			from: value.from,
			to: value.to,
			weight: value.weight,
			domain: value.domain,
			form: form.into(),
			timestamp: value.timestamp,
		}
	}
}

#[cfg(test)]
mod test {
	use super::{Term, TermForm};

	#[test]
	fn should_convert_term_to_bytes_and_back() {
		let term = Term {
			from: "did:eth:pkh:0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1".to_owned(),
			to: "did:eth:pkh:0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned(),
			weight: 50.,
			domain: 67834578,
			form: TermForm::Trust,
			timestamp: 0,
		};

		let bytes = term.clone().into_bytes().unwrap();
		let rec_term = Term::from_bytes(bytes).unwrap();

		assert_eq!(term, rec_term);
	}
}
