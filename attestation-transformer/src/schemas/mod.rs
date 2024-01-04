use crate::{did::Did, error::AttTrError, term::Term};
use secp256k1::PublicKey;
use serde_derive::{Deserialize, Serialize};

pub mod approve;
pub mod disapprove;
pub mod follow;
pub mod status;

#[derive(Deserialize, Serialize, Clone)]
pub struct Proof {
	signature: String,
}

impl Proof {
	pub fn get_signature(&self) -> String {
		self.signature.clone()
	}
}

pub trait Validation {
	fn validate(&self) -> Result<(PublicKey, Did), AttTrError>;
}

pub trait IntoTerm: Validation {
	const DOMAIN: u32;

	fn into_term(self) -> Result<Term, AttTrError>;
}

pub enum SchemaType {
	Follow,
	AuditApprove,
	AuditDisapprove,
	EndorseCredential,
}

impl From<u32> for SchemaType {
	fn from(value: u32) -> Self {
		match value {
			1 => Self::Follow,
			2 => Self::AuditApprove,
			3 => Self::AuditDisapprove,
			4 => Self::EndorseCredential,
			_ => panic!("Invalid Schema type"),
		}
	}
}
