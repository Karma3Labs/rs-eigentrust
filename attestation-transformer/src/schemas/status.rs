use crate::did::Schema;
use crate::{did::Did, error::AttTrError, term::Term, utils::address_from_ecdsa_key};
use secp256k1::rand::thread_rng;
use secp256k1::{generate_keypair, Message, Secp256k1};
use serde_derive::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

use super::{IntoTerm, Proof, Validation};

#[derive(Deserialize, Serialize, Clone)]
pub enum CurrentStatus {
	Endorsed,
	Disputed,
}

impl Into<u8> for CurrentStatus {
	fn into(self) -> u8 {
		match self {
			Self::Endorsed => 1,
			Self::Disputed => 0,
		}
	}
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CredentialSubject {
	id: String,
	current_status: CurrentStatus,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EndorseCredential {
	#[serde(alias = "type")]
	kind: String,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

#[cfg(test)]
impl EndorseCredential {
	pub fn new(id: String, current_status: CurrentStatus) -> Self {
		let did = Did::parse_pkh_eth(id.clone()).unwrap();
		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		keccak.update(&[current_status.clone().into()]);
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

		let kind = "EndorsementCredential".to_string();
		let addr = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", hex::encode(addr));
		let cs = CredentialSubject { id, current_status };
		let proof = Proof { signature: encoded_sig };

		EndorseCredential { kind, issuer, credential_subject: cs, proof }
	}
}

impl Validation for EndorseCredential {
	fn get_trimmed_signature(&self) -> String {
		self.proof.get_signature().trim_start_matches("0x").to_owned()
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError> {
		let did = Did::parse_pkh_eth(self.credential_subject.id.clone())?;
		let mut bytes = Vec::new();
		bytes.extend_from_slice(&did.key);
		bytes.push(self.credential_subject.current_status.clone().into());

		Ok(bytes)
	}
}

impl IntoTerm for EndorseCredential {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let pk = self.validate()?;

		let from_address = address_from_ecdsa_key(&pk);
		let from_did: String = Did::new(Schema::PkhEth, from_address).into();
		let weight = 50;

		Ok(Term::new(
			from_did,
			self.credential_subject.id,
			weight,
			Self::DOMAIN,
			true,
		))
	}
}

#[cfg(test)]
mod test {
	use crate::schemas::status::{CredentialSubject, EndorseCredential};
	use crate::schemas::{Proof, Validation};
	use crate::utils::address_from_ecdsa_key;
	use crate::{did::Did, schemas::status::CurrentStatus};
	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_validate_endorse_credential() {
		let did_string = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let did = Did::parse_pkh_eth(did_string.clone()).unwrap();
		let current_status = CurrentStatus::Endorsed;

		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		keccak.update(&[current_status.clone().into()]);
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
		let cs = CredentialSubject { id: did_string, current_status };
		let proof = Proof { signature: sig_string };

		let follow_schema = EndorseCredential { kind, issuer, credential_subject: cs, proof };

		let rec_pk = follow_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
