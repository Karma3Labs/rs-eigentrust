use crate::{error::AttTrError, term::Term};
use secp256k1::PublicKey;

pub mod approve;
pub mod disapprove;
pub mod follow;
pub mod status;

pub trait Validation {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError>;
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
