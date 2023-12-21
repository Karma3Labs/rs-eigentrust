use super::{IntoTerm, Proof, Validation};
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
struct CredentialSubject {
	id: String,
	status_reason: StatusReason,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AuditDisapproveSchema {
	#[serde(rename(serialize = "type"))]
	kind: String,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

#[cfg(test)]
impl AuditDisapproveSchema {
	fn new(id: String, status_reason: StatusReason) -> Self {
		let did = Did::parse(id.clone()).unwrap();
		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		keccak.update(&[status_reason.clone().into()]);
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, pk) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, sig_bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut bytes = Vec::new();
		bytes.extend_from_slice(&sig_bytes);
		bytes.extend_from_slice(&rec_id_i32.to_be_bytes());
		let encoded_sig = hex::encode(bytes);

		let kind = "AuditReportDisapproveCredential".to_string();
		let address = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", address);
		let status_reason = StatusReason::Incomplete;
		let cs = CredentialSubject { id, status_reason };
		let proof = Proof { signature: encoded_sig };

		AuditDisapproveSchema { kind, issuer, credential_subject: cs, proof }
	}
}

impl Validation for AuditDisapproveSchema {
	fn validate(&self) -> Result<(PublicKey, Did), AttTrError> {
		let did = Did::parse(self.credential_subject.id.clone())?;
		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		keccak.update(&[self.credential_subject.status_reason.clone().into()]);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;

		let sig_bytes = hex::decode(self.proof.signature.clone())
			.map_err(|_| AttTrError::SerialisationError)?;
		let mut rs_bytes = [0; 64];
		rs_bytes[..64].copy_from_slice(&sig_bytes[..64]);
		let rec_id: i32 = i32::from_be_bytes(sig_bytes[64..].try_into().unwrap());
		let signature = RecoverableSignature::from_compact(
			&rs_bytes,
			RecoveryId::from_i32(rec_id).map_err(|x| AttTrError::VerificationError(x))?,
		)
		.map_err(|x| AttTrError::VerificationError(x))?;
		let pk = signature.recover(&message).map_err(|x| AttTrError::VerificationError(x))?;

		let secp = Secp256k1::verification_only();
		secp.verify_ecdsa(&message, &signature.to_standard(), &pk)
			.map_err(|x| AttTrError::VerificationError(x))?;
		Ok((pk, did))
	}
}

impl IntoTerm for AuditDisapproveSchema {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let (pk, did) = self.validate()?;

		let from_address = address_from_ecdsa_key(&pk);
		let to_address = hex::encode(&did.key);
		let weight = match self.credential_subject.status_reason {
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
	use crate::{
		did::Did,
		schemas::{disapprove::CredentialSubject, Proof, Validation},
		utils::address_from_ecdsa_key,
	};

	use super::{AuditDisapproveSchema, StatusReason};
	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_validate_audit_disapprove_schema() {
		let did_string = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let did = Did::parse(did_string.clone()).unwrap();
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
		let (rec_id, sig_bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut bytes = Vec::new();
		bytes.extend_from_slice(&sig_bytes);
		bytes.extend_from_slice(&rec_id_i32.to_be_bytes());
		let sig_string = hex::encode(bytes);

		let kind = "AuditReportDisapproveCredential".to_string();
		let address = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", address);
		let status_reason = StatusReason::Incomplete;
		let cs = CredentialSubject { id: did_string, status_reason };
		let proof = Proof { signature: sig_string };

		let aa_schema = AuditDisapproveSchema { kind, issuer, credential_subject: cs, proof };

		let (rec_pk, _) = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
