use crate::{error::AttTrError, term::Term};
use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	Message, PublicKey, Secp256k1,
};
use serde_derive::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub mod approve;
pub mod disapprove;
pub mod status;
pub mod trust;

#[derive(Deserialize, Serialize, Clone)]
pub struct Proof {
	signature: String,
}

impl Proof {
	pub fn new(signature: String) -> Self {
		Self { signature }
	}

	pub fn get_signature(&self) -> String {
		self.signature.clone()
	}
}

pub trait Validation {
	fn get_trimmed_signature(&self) -> String;

	fn validate(&self) -> Result<PublicKey, AttTrError> {
		let sig_bytes = hex::decode(self.get_trimmed_signature())
			.map_err(|_| AttTrError::SerialisationError)?;

		let mut rs_bytes = [0; 64];
		rs_bytes.copy_from_slice(&sig_bytes[..64]);
		let rec_id: i32 = match i32::from(sig_bytes[64]) {
			0 => 0,
			1 => 1,
			27 => 0,
			28 => 1,
			_ => return Err(AttTrError::ParseError),
		};

		let rec_id_p =
			RecoveryId::from_i32(rec_id).map_err(|x| AttTrError::VerificationError(x))?;

		let signature = RecoverableSignature::from_compact(&rs_bytes, rec_id_p)
			.map_err(|x| AttTrError::VerificationError(x))?;

		let mut keccak = Keccak256::default();
		keccak.update(&self.get_message()?);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;
		let pk = signature.recover(&message).map_err(|x| AttTrError::VerificationError(x))?;

		let secp = Secp256k1::verification_only();
		secp.verify_ecdsa(&message, &signature.to_standard(), &pk)
			.map_err(|x| AttTrError::VerificationError(x))?;

		Ok(pk)
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError>;
}

pub trait IntoTerm: Validation {
	fn into_term(self) -> Result<Vec<Term>, AttTrError>;
}

pub enum SchemaType {
	AuditApprove,
	AuditDisapprove,
	StatusCredential,
	TrustCredential,
}

impl From<u32> for SchemaType {
	fn from(value: u32) -> Self {
		match value {
			2 => Self::AuditApprove,
			3 => Self::AuditDisapprove,
			4 => Self::StatusCredential,
			5 => Self::TrustCredential,
			_ => panic!("Invalid Schema type"),
		}
	}
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Domain {
	Honesty,
	SoftwareDevelopment,
	SoftwareSecurity,
}

impl Into<u8> for Domain {
	fn into(self) -> u8 {
		match self {
			Self::Honesty => 0,
			Self::SoftwareDevelopment => 1,
			Self::SoftwareSecurity => 2,
		}
	}
}
