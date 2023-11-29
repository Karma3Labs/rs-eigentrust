use super::term::Validation;
use crate::{
	error::AttTrError,
	term::{IntoTerm, Term},
	utils::{address_from_ecdsa_key, address_to_did},
};
use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	Message, PublicKey, Secp256k1,
};
use serde_derive::Deserialize;
use sha3::{digest::Digest, Keccak256};

pub enum SchemaType {
	Follow,
	AuditApprove,
	AuditDisapprove,
}

impl From<u32> for SchemaType {
	fn from(value: u32) -> Self {
		match value {
			1 => Self::Follow,
			2 => Self::AuditApprove,
			3 => Self::AuditDisapprove,
			_ => panic!("Invalid Schema type"),
		}
	}
}

#[derive(Deserialize, Clone)]
pub enum Scope {
	Reviewer,
	Developer,
	Auditor,
}

impl Into<u8> for Scope {
	fn into(self) -> u8 {
		match self {
			Self::Reviewer => 0,
			Self::Developer => 1,
			Self::Auditor => 2,
		}
	}
}

#[derive(Deserialize)]
pub struct FollowSchema {
	id: String,
	is_trustworthy: bool,
	scope: Scope,
	sig: (i32, [u8; 32], [u8; 32]),
}

impl Validation for FollowSchema {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError> {
		let mut keccak = Keccak256::default();
		keccak.update(self.id.as_bytes());
		keccak.update(&[self.is_trustworthy.into()]);
		keccak.update(&[self.scope.clone().into()]);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;

		let mut rs_bytes = [0; 64];
		rs_bytes[..32].copy_from_slice(&self.sig.1);
		rs_bytes[32..].copy_from_slice(&self.sig.2);
		let signature = RecoverableSignature::from_compact(
			&rs_bytes,
			RecoveryId::from_i32(self.sig.0).map_err(|x| AttTrError::VerificationError(x))?,
		)
		.map_err(|x| AttTrError::VerificationError(x))?;
		let pk = signature.recover(&message).map_err(|x| AttTrError::VerificationError(x))?;

		let secp = Secp256k1::verification_only();
		Ok((
			pk,
			secp.verify_ecdsa(&message, &signature.to_standard(), &pk).is_ok(),
		))
	}
}

impl IntoTerm for FollowSchema {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let (pk, valid) = self.validate()?;
		assert!(valid);

		let address = address_from_ecdsa_key(&pk);
		let sender_did = address_to_did(&address);

		let weight = 50;

		Ok(Term::new(sender_did, self.id, weight, Self::DOMAIN, true))
	}
}

#[derive(Deserialize)]
pub struct AuditApproveSchema {
	id: String,
	sig: (i32, [u8; 32], [u8; 32]),
}

impl Validation for AuditApproveSchema {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError> {
		let mut keccak = Keccak256::default();
		keccak.update(self.id.as_bytes());
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;

		let mut rs_bytes = [0; 64];
		rs_bytes[..32].copy_from_slice(&self.sig.1);
		rs_bytes[32..].copy_from_slice(&self.sig.2);
		let signature = RecoverableSignature::from_compact(
			&rs_bytes,
			RecoveryId::from_i32(self.sig.0).map_err(|x| AttTrError::VerificationError(x))?,
		)
		.map_err(|x| AttTrError::VerificationError(x))?;
		let pk = signature.recover(&message).map_err(|x| AttTrError::VerificationError(x))?;

		let secp = Secp256k1::verification_only();
		Ok((
			pk,
			secp.verify_ecdsa(&message, &signature.to_standard(), &pk).is_ok(),
		))
	}
}

impl IntoTerm for AuditApproveSchema {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let (pk, valid) = self.validate()?;
		assert!(valid);

		let address = address_from_ecdsa_key(&pk);
		let sender_did = address_to_did(&address);

		let weight = 50;

		Ok(Term::new(sender_did, self.id, weight, Self::DOMAIN, true))
	}
}

#[derive(Deserialize, Clone)]
enum StatusReason {
	Unreliable,
	Scam,
	Incomplete,
}

impl Into<u8> for StatusReason {
	fn into(self) -> u8 {
		match self {
			Self::Unreliable => 0,
			Self::Scam => 1,
			Self::Incomplete => 2,
		}
	}
}

#[derive(Deserialize)]
pub struct AuditDisapproveSchema {
	id: String,
	status_reason: StatusReason,
	sig: (i32, [u8; 32], [u8; 32]),
}

impl Validation for AuditDisapproveSchema {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError> {
		let mut keccak = Keccak256::default();
		keccak.update(self.id.as_bytes());
		keccak.update(&[self.status_reason.clone().into()]);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;

		let mut rs_bytes = [0; 64];
		rs_bytes[..32].copy_from_slice(&self.sig.1);
		rs_bytes[32..].copy_from_slice(&self.sig.2);
		let signature = RecoverableSignature::from_compact(
			&rs_bytes,
			RecoveryId::from_i32(self.sig.0).map_err(|x| AttTrError::VerificationError(x))?,
		)
		.map_err(|x| AttTrError::VerificationError(x))?;
		let pk = signature.recover(&message).map_err(|x| AttTrError::VerificationError(x))?;

		let secp = Secp256k1::verification_only();
		Ok((
			pk,
			secp.verify_ecdsa(&message, &signature.to_standard(), &pk).is_ok(),
		))
	}
}

impl IntoTerm for AuditDisapproveSchema {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let (pk, valid) = self.validate()?;
		assert!(valid);

		let address = address_from_ecdsa_key(&pk);
		let sender_did = address_to_did(&address);

		let weight = match self.status_reason {
			StatusReason::Unreliable => 10,
			StatusReason::Scam => 50,
			StatusReason::Incomplete => 100,
		};

		Ok(Term::new(sender_did, self.id, weight, Self::DOMAIN, false))
	}
}
