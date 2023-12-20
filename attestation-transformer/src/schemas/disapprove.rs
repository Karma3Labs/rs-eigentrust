use super::{IntoTerm, Validation};
use crate::{did::Did, error::AttTrError, term::Term, utils::address_from_ecdsa_key};
use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	generate_keypair,
	rand::thread_rng,
	Message, PublicKey, Secp256k1,
};
use serde_derive::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

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
	id: Did,
	status_reason: StatusReason,
	sig: (i32, [u8; 32], [u8; 32]),
}

#[cfg(test)]
impl AuditDisapproveSchema {
	fn new(id: String, status_reason: StatusReason) -> Self {
		let did = Did::parse(id).unwrap();
		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
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

		AuditDisapproveSchema { id: did, status_reason, sig: (rec_id_i32, r_bytes, s_bytes) }
	}
}

impl Validation for AuditDisapproveSchema {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError> {
		let mut keccak = Keccak256::default();
		keccak.update(&self.id.key);
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

		let from_address = address_from_ecdsa_key(&pk);
		let to_address = hex::encode(&self.id.key);
		let weight = match self.status_reason {
			StatusReason::Unreliable => 10,
			StatusReason::Scam => 50,
			StatusReason::Incomplete => 100,
		};

		Ok(Term::new(
			from_address,
			to_address,
			weight,
			Self::DOMAIN,
			false,
		))
	}
}

#[cfg(test)]
mod test {
	use crate::{did::Did, schemas::Validation};

	use super::{AuditDisapproveSchema, StatusReason};
	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_validate_audit_disapprove_schema() {
		let did_string = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let did = Did::parse(did_string).unwrap();
		let status_reason = StatusReason::Scam;

		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
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
			AuditDisapproveSchema { id: did, status_reason, sig: (rec_id_i32, r_bytes, s_bytes) };

		let (rec_pk, valid) = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
		assert!(valid);
	}
}
