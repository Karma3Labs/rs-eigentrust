use crate::{error::AttTrError, term::Term};
use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	Message, PublicKey, Secp256k1,
};
use serde_derive::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub mod security;
pub mod status;
pub mod trust;

#[derive(Deserialize, Serialize, Clone)]
pub struct Proof {
	// TODO(ek): Fix this
	signature: Option<String>,
}

impl Proof {
	pub fn new(signature: String) -> Self {
		Self { signature: Some(signature) }
	}

	pub fn get_signature(&self) -> String {
		self.signature.clone().unwrap_or_default()
	}
}

pub trait Validation {
	fn get_trimmed_signature(&self) -> String;

	fn validate(&self) -> Result<PublicKey, AttTrError> {
		let sig_bytes = hex::decode(self.get_trimmed_signature()).map_err(AttTrError::HexError)?;
		let mut rs_bytes = [0; 64];
		rs_bytes.copy_from_slice(&sig_bytes[..64]);
		let rec_id: i32 = match i32::from(sig_bytes[64]) {
			0 => 0,
			1 => 1,
			27 => 0,
			28 => 1,
			_ => return Err(AttTrError::ParseError),
		};

		let rec_id_p = RecoveryId::from_i32(rec_id).map_err(AttTrError::SigVerificationError)?;

		let signature = RecoverableSignature::from_compact(&rs_bytes, rec_id_p)
			.map_err(AttTrError::SigVerificationError)?;

		let mut keccak = Keccak256::default();
		keccak.update(&self.get_message()?);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(AttTrError::SigVerificationError)?;
		let pk = signature.recover(&message).map_err(AttTrError::SigVerificationError)?;

		let secp = Secp256k1::verification_only();
		secp.verify_ecdsa(&message, &signature.to_standard(), &pk)
			.map_err(AttTrError::SigVerificationError)?;

		Ok(pk)
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError>;
}

pub trait IntoTerm: Validation {
	fn into_term(self, timestamp: u64) -> Result<Vec<Term>, AttTrError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaTypeError {
	#[error("invalid schema type number {0}")]
	InvalidNumber(u32),
}

pub enum SchemaType {
	SecurityCredential,
	StatusCredential,
	TrustCredential,
}

// impl From<u32> for SchemaType {
// 	fn from(value: u32) -> Self {
// 		match value {
// 			0 => Self::SecurityCredential,
// 			1 => Self::StatusCredential,
// 			2 => Self::TrustCredential,
// 			_ => panic!("Invalid Schema type"),
// 		}
// 	}
// }

impl TryFrom<u32> for SchemaType {
	type Error = SchemaTypeError;

	fn try_from(value: u32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::SecurityCredential),
			1 => Ok(Self::StatusCredential),
			2 => Ok(Self::TrustCredential),
			_ => Err(SchemaTypeError::InvalidNumber(value)),
		}
	}
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Domain {
	Honesty,
	#[serde(rename = "Software development")]
	SoftwareDevelopment,
	#[serde(rename = "Software security")]
	SoftwareSecurity,
}

impl From<Domain> for u8 {
	fn from(value: Domain) -> Self {
		match value {
			Domain::Honesty => 0,
			Domain::SoftwareDevelopment => 1,
			Domain::SoftwareSecurity => 2,
		}
	}
}

impl From<Domain> for u32 {
	fn from(value: Domain) -> Self {
		match value {
			Domain::Honesty => 0,
			Domain::SoftwareDevelopment => 1,
			Domain::SoftwareSecurity => 2,
		}
	}
}
