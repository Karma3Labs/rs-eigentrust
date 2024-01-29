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

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
	from: String,
	to: String,
	weight: f32,
	domain: u32,
	form: TermForm,
	timestamp: u64,
}

impl Term {
	pub fn new(
		from: String, to: String, weight: f32, domain: u32, is_trust: bool, timestamp: u64,
	) -> Term {
		Term {
			from,
			to,
			weight,
			domain,
			form: if is_trust { TermForm::Trust } else { TermForm::Distrust },
			timestamp,
		}
	}

	pub fn into_bytes(self) -> Result<Vec<u8>, AttTrError> {
		let mut bytes = Vec::new();

		let from_bytes = self.from.as_bytes();
		let to_bytes = self.to.as_bytes();
		let weight_bytes = self.weight.to_be_bytes();
		let domain_bytes = self.domain.to_be_bytes();
		let form_byte: u8 = self.form.into();
		let timestamp_bytes = self.timestamp.to_be_bytes();

		bytes.extend_from_slice(&from_bytes);
		bytes.extend_from_slice(&to_bytes);
		bytes.extend_from_slice(&weight_bytes);
		bytes.extend_from_slice(&domain_bytes);
		bytes.push(form_byte);
		bytes.extend_from_slice(&timestamp_bytes);

		Ok(bytes)
	}

	pub fn from_bytes(mut bytes: Vec<u8>) -> Result<Self, AttTrError> {
		let term: Term = match bytes.len() {
			// 54 + 49 + 4 + 4 + 1 + 8 = 120
			// 54: did:pkh:eth:0x152d4dd8afe95f7c38103d7460befbed07dedd8f - from
			// 49: snap://0x9dc6c239a0f3abad2094cd6891cdc56cdf8994f8 - to
			// 4: f32 - weight
			// 4: u32 - domain
			// 1: u8 - form
			// 8: u63 - timestamp
			120 => {
				let from_bytes: Vec<u8> = bytes.drain(..54).collect();
				let to_bytes: Vec<u8> = bytes.drain(..49).collect();
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
				let form_byte: u8 = bytes.remove(0);
				let timestamp_bytes = bytes
					.drain(..8)
					.collect::<Vec<u8>>()
					.try_into()
					.map_err(|_| AttTrError::SerialisationError)?;

				let from =
					String::from_utf8(from_bytes).map_err(|_| AttTrError::SerialisationError)?;
				let to = String::from_utf8(to_bytes).map_err(|_| AttTrError::SerialisationError)?;
				let weight = f32::from_be_bytes(weight_bytes);
				let domain = u32::from_be_bytes(domain_bytes);
				let form = TermForm::from(form_byte);
				let timestamp = u64::from_be_bytes(timestamp_bytes);

				Term { from, to, weight, domain, form, timestamp }
			},
			// 54 + 54 + 4 + 4 + 1 + 8 = 125
			// 54: did:pkh:eth:0x152d4dd8afe95f7c38103d7460befbed07dedd8f - from
			// 54: did:pkh:eth:0x152d4dd8afe95f7c38103d7460befbed07dedd8f - to
			// 4: f32 - weight
			// 4: u32 - domain
			// 1: u8 - form
			// 8: u63 - timestamp
			125 => {
				let from_bytes: Vec<u8> = bytes.drain(..54).collect();
				let to_bytes: Vec<u8> = bytes.drain(..54).collect();
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
				let form_byte: u8 = bytes.remove(0);
				let timestamp_bytes = bytes
					.drain(..8)
					.collect::<Vec<u8>>()
					.try_into()
					.map_err(|_| AttTrError::SerialisationError)?;

				let from =
					String::from_utf8(from_bytes).map_err(|_| AttTrError::SerialisationError)?;
				let to = String::from_utf8(to_bytes).map_err(|_| AttTrError::SerialisationError)?;
				let weight = f32::from_be_bytes(weight_bytes);
				let domain = u32::from_be_bytes(domain_bytes);
				let form = TermForm::from(form_byte);
				let timestamp = u64::from_be_bytes(timestamp_bytes);

				Term { from, to, weight, domain, form, timestamp }
			},
			_ => return Err(AttTrError::SerialisationError),
		};

		Ok(term)
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
			timestamp: self.timestamp,
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
