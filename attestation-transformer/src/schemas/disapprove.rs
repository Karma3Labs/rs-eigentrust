use super::{IntoTerm, Proof, Validation};
use crate::{
	did::{Did, Schema},
	error::AttTrError,
	term::Term,
	utils::address_from_ecdsa_key,
};
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
#[serde(rename_all = "camelCase")]
struct CredentialSubject {
	id: String,
	status_reason: StatusReason,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuditDisapproveSchema {
	#[serde(alias = "type")]
	kind: String,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

#[cfg(test)]
impl AuditDisapproveSchema {
	fn new(id: String, status_reason: StatusReason) -> Self {
		let did = Did::parse_snap(id.clone()).unwrap();
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
		bytes.push(rec_id_i32.to_le_bytes()[0]);
		let encoded_sig = hex::encode(bytes);

		let kind = "AuditReportDisapproveCredential".to_string();
		let addr = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", hex::encode(addr));
		let status_reason = StatusReason::Incomplete;
		let cs = CredentialSubject { id, status_reason };
		let proof = Proof { signature: encoded_sig };

		AuditDisapproveSchema { kind, issuer, credential_subject: cs, proof }
	}
}

impl Validation for AuditDisapproveSchema {
	fn get_trimmed_signature(&self) -> String {
		self.proof.get_signature().trim_start_matches("0x").to_owned()
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError> {
		let did = Did::parse_snap(self.credential_subject.id.clone())?;
		let mut bytes = Vec::new();
		bytes.extend_from_slice(&did.key);
		bytes.push(self.credential_subject.status_reason.clone().into());

		Ok(bytes)
	}
}

impl IntoTerm for AuditDisapproveSchema {
	fn into_term(self) -> Result<Term, AttTrError> {
		let pk = self.validate()?;

		let from_address = address_from_ecdsa_key(&pk);
		let from_did: String = Did::new(Schema::PkhEth, from_address).into();
		let weight = match self.credential_subject.status_reason {
			StatusReason::Unreliable => 10.,
			StatusReason::Scam => 50.,
			StatusReason::Incomplete => 100.,
		};
		let domain = 1;

		Ok(Term::new(
			from_did, self.credential_subject.id, weight, domain, false,
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
		let did_string = "snap://90f8bf6a47".to_owned();
		let did = Did::parse_snap(did_string.clone()).unwrap();
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
		let rec_id = rec_id.to_i32().to_le_bytes()[0];

		let mut bytes = Vec::new();
		bytes.extend_from_slice(&sig_bytes);
		bytes.push(rec_id);
		let sig_string = hex::encode(bytes);

		let kind = "AuditReportDisapproveCredential".to_string();
		let address = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", hex::encode(address));
		let cs = CredentialSubject { id: did_string, status_reason };
		let proof = Proof { signature: sig_string };

		let aa_schema = AuditDisapproveSchema { kind, issuer, credential_subject: cs, proof };

		let rec_pk = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
