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
struct CredentialSubject {
	id: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuditApproveSchema {
	#[serde(alias = "type")]
	kind: String,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

#[cfg(test)]
impl AuditApproveSchema {
	fn new(id: String) -> Self {
		let did = Did::parse_snap(id.clone()).unwrap();
		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
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
		bytes.push(rec_id_i32.to_le_bytes()[0]);
		let encoded_sig = hex::encode(bytes);

		let kind = "AuditReportApproveCredential".to_string();
		let address = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", address);
		let cs = CredentialSubject { id };
		let proof = Proof { signature: encoded_sig };

		AuditApproveSchema { kind, issuer, credential_subject: cs, proof }
	}
}

impl Validation for AuditApproveSchema {
	fn validate(&self) -> Result<(PublicKey, Did), AttTrError> {
		let did = Did::parse_snap(self.credential_subject.id.clone())?;
		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;

		let sig_bytes = hex::decode(self.proof.signature.trim_start_matches("0x"))
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

		let pk = signature.recover(&message).map_err(|x| AttTrError::VerificationError(x))?;

		let secp = Secp256k1::verification_only();
		secp.verify_ecdsa(&message, &signature.to_standard(), &pk)
			.map_err(|x| AttTrError::VerificationError(x))?;
		Ok((pk, did))
	}
}

impl IntoTerm for AuditApproveSchema {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let (pk, did) = self.validate()?;

		let from_address = address_from_ecdsa_key(&pk);
		let to_address = hex::encode(&did.key);
		let weight = 50;

		Ok(Term::new(
			from_address,
			to_address,
			weight,
			Self::DOMAIN,
			true,
		))
	}
}

#[cfg(test)]
mod test {
	use crate::{
		did::Did,
		schemas::{approve::CredentialSubject, Proof, Validation},
		utils::address_from_ecdsa_key,
	};

	use super::AuditApproveSchema;
	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_validate_audit_approve_schema() {
		let did_string = "snap://90f8bf6a47".to_owned();
		let did = Did::parse_snap(did_string.clone()).unwrap();

		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		let digest = keccak.finalize();

		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let rng = &mut thread_rng();
		let (sk, pk) = generate_keypair(rng);
		let secp = Secp256k1::new();
		let res = secp.sign_ecdsa_recoverable(&message, &sk);
		let (rec_id, sig_bytes) = res.serialize_compact();
		let rec_id = rec_id.to_i32().to_le_bytes()[0];

		let mut bytes = Vec::new();
		bytes.extend_from_slice(&sig_bytes);
		bytes.push(rec_id);
		let sig_string = hex::encode(bytes);

		let kind = "AuditReportApproveCredential".to_string();
		let issuer = format!("did:pkh:eth:{}", address_from_ecdsa_key(&pk));
		let cs = CredentialSubject { id: did_string };
		let proof = Proof { signature: sig_string };

		let aa_schema = AuditApproveSchema { kind, issuer, credential_subject: cs, proof };

		let (rec_pk, _) = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
