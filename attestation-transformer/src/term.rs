use crate::error::AttTrError;
use proto_buf::transformer::{Form, TermObject};

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

	pub fn into_bytes(self) -> Result<Vec<u8>, AttTrError> {
		let mut bytes = Vec::new();

		let from_bytes = hex::decode(self.from).map_err(|_| AttTrError::SerialisationError)?;
		let to_bytes = hex::decode(self.to).map_err(|_| AttTrError::SerialisationError)?;
		let weight_bytes = self.weight.to_be_bytes();
		let domain_bytes = self.domain.to_be_bytes();
		let form_byte: u8 = self.form.into();

		bytes.extend_from_slice(&from_bytes);
		bytes.extend_from_slice(&to_bytes);
		bytes.extend_from_slice(&weight_bytes);
		bytes.extend_from_slice(&domain_bytes);
		bytes.push(form_byte);

		Ok(bytes)
	}

	pub fn from_bytes(mut bytes: Vec<u8>) -> Result<Self, AttTrError> {
		if bytes.len() != 49 {
			return Err(AttTrError::SerialisationError);
		}
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

		let from = hex::encode(from_bytes);
		let to = hex::encode(to_bytes);
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

#[cfg(test)]
mod test {
	use super::{Term, TermForm};

	#[test]
	fn should_convert_term_to_bytes_and_back() {
		let term = Term {
			from: "90f8bf6a479f320ead074411a4b0e7944ea8c9c1".to_owned(),
			to: "90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned(),
			weight: 50,
			domain: 67834578,
			form: TermForm::Trust,
		};

		let bytes = term.clone().into_bytes().unwrap();
		let rec_term = Term::from_bytes(bytes).unwrap();

		assert_eq!(term, rec_term);
	}
}
