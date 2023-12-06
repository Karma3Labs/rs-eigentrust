use super::term::Validation;
use crate::{
	error::AttTrError,
	term::{IntoTerm, Term},
	utils::address_from_ecdsa_key,
};
use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	Message, PublicKey, Secp256k1,
};
use serde_derive::{Deserialize, Serialize};
use sha3::{digest::Digest, Keccak256};

#[cfg(test)]
use secp256k1::{generate_keypair, rand::thread_rng};

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

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Deserialize, Serialize, Clone)]
pub struct FollowSchema {
	id: String,
	is_trustworthy: bool,
	scope: Scope,
	sig: (i32, [u8; 32], [u8; 32]),
}

#[cfg(test)]
impl FollowSchema {
	pub fn new(id: String, is_trustworthy: bool, scope: Scope) -> Self {
		let mut keccak = Keccak256::default();
		keccak.update(id.as_bytes());
		keccak.update(&[is_trustworthy.into()]);
		keccak.update(&[scope.clone().into()]);
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, _) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut r_bytes = [0u8; 32];
		let mut s_bytes = [0u8; 32];
		r_bytes.copy_from_slice(&bytes[..32]);
		s_bytes.copy_from_slice(&bytes[32..]);

		FollowSchema { id, is_trustworthy, scope, sig: (rec_id_i32, r_bytes, s_bytes) }
	}
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
		let weight = 50;

		Ok(Term::new(address, self.id, weight, Self::DOMAIN, true))
	}
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AuditApproveSchema {
	id: String,
	sig: (i32, [u8; 32], [u8; 32]),
}

#[cfg(test)]
impl AuditApproveSchema {
	fn new(id: String) -> Self {
		let mut keccak = Keccak256::default();
		keccak.update(id.as_bytes());
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, _) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut r_bytes = [0u8; 32];
		let mut s_bytes = [0u8; 32];
		r_bytes.copy_from_slice(&bytes[..32]);
		s_bytes.copy_from_slice(&bytes[32..]);

		AuditApproveSchema { id, sig: (rec_id_i32, r_bytes, s_bytes) }
	}
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
		let weight = 50;

		Ok(Term::new(address, self.id, weight, Self::DOMAIN, true))
	}
}

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Deserialize, Serialize, Clone)]
pub struct AuditDisapproveSchema {
	id: String,
	status_reason: StatusReason,
	sig: (i32, [u8; 32], [u8; 32]),
}

#[cfg(test)]
impl AuditDisapproveSchema {
	fn new(id: String, status_reason: StatusReason) -> Self {
		let mut keccak = Keccak256::default();
		keccak.update(id.as_bytes());
		keccak.update(&[status_reason.clone().into()]);
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, _) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut r_bytes = [0u8; 32];
		let mut s_bytes = [0u8; 32];
		r_bytes.copy_from_slice(&bytes[..32]);
		s_bytes.copy_from_slice(&bytes[32..]);

		AuditDisapproveSchema { id, status_reason, sig: (rec_id_i32, r_bytes, s_bytes) }
	}
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
		let weight = match self.status_reason {
			StatusReason::Unreliable => 10,
			StatusReason::Scam => 50,
			StatusReason::Incomplete => 100,
		};

		Ok(Term::new(address, self.id, weight, Self::DOMAIN, false))
	}
}

#[cfg(test)]
mod test {
	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	use crate::{
		schemas::{AuditApproveSchema, AuditDisapproveSchema, StatusReason},
		term::Validation,
	};

	use super::{FollowSchema, Scope};

	#[test]
	fn should_validate_follow_schema() {
		let id = "90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let is_trustworthy = true;
		let scope = Scope::Auditor;

		let mut keccak = Keccak256::default();
		keccak.update(id.as_bytes());
		keccak.update(&[is_trustworthy.into()]);
		keccak.update(&[scope.clone().into()]);
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, pk) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut r_bytes = [0u8; 32];
		let mut s_bytes = [0u8; 32];
		r_bytes.copy_from_slice(&bytes[..32]);
		s_bytes.copy_from_slice(&bytes[32..]);

		let follow_schema =
			FollowSchema { id, is_trustworthy, scope, sig: (rec_id_i32, r_bytes, s_bytes) };

		let (rec_pk, valid) = follow_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
		assert!(valid);
	}
	#[test]
	fn should_validate_audit_approve_schema() {
		let id = "90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();

		let mut keccak = Keccak256::default();
		keccak.update(id.as_bytes());
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, pk) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut r_bytes = [0u8; 32];
		let mut s_bytes = [0u8; 32];
		r_bytes.copy_from_slice(&bytes[..32]);
		s_bytes.copy_from_slice(&bytes[32..]);

		let aa_schema = AuditApproveSchema { id, sig: (rec_id_i32, r_bytes, s_bytes) };

		let (rec_pk, valid) = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
		assert!(valid);
	}
	#[test]
	fn should_validate_audit_disapprove_schema() {
		let id = "90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let status_reason = StatusReason::Scam;

		let mut keccak = Keccak256::default();
		keccak.update(id.as_bytes());
		keccak.update(&[status_reason.clone().into()]);
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, pk) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut r_bytes = [0u8; 32];
		let mut s_bytes = [0u8; 32];
		r_bytes.copy_from_slice(&bytes[..32]);
		s_bytes.copy_from_slice(&bytes[32..]);

		let aa_schema =
			AuditDisapproveSchema { id, status_reason, sig: (rec_id_i32, r_bytes, s_bytes) };

		let (rec_pk, valid) = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
		assert!(valid);
	}
}
