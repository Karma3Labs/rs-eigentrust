use serde_derive::{Deserialize, Serialize};

use mm_spd_vc::OneOrMore;

use crate::did::Did;
use crate::error::AttTrError;
use crate::schemas::{Domain, IntoTerm, Proof, Validation};
use crate::term::{Term, TermForm};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum CurrentStatus {
	Endorsed,
	Disputed,
}

impl From<CurrentStatus> for u8 {
	fn from(value: CurrentStatus) -> Self {
		match value {
			CurrentStatus::Endorsed => 1,
			CurrentStatus::Disputed => 0,
		}
	}
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StatusReason {
	#[serde(rename = "type")]
	kind: Option<String>,
	value: OneOrMore<String>,
	lang: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSubject {
	id: String,
	current_status: CurrentStatus,
	status_reason: Option<StatusReason>,
}

impl CredentialSubject {
	pub fn new(
		id: String, current_status: CurrentStatus, status_reason: Option<StatusReason>,
	) -> Self {
		Self { id, current_status, status_reason }
	}
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatusSchema {
	#[serde(rename = "type")]
	kind: OneOrMore<String>,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

impl StatusSchema {
	pub fn new(
		kind: OneOrMore<String>, issuer: String, credential_subject: CredentialSubject,
		proof: Proof,
	) -> Self {
		Self { kind, issuer, credential_subject, proof }
	}

	pub fn get_issuer(&self) -> String {
		self.issuer.clone()
	}
}

impl Validation for StatusSchema {
	fn get_trimmed_signature(&self) -> String {
		self.proof.get_signature().trim_start_matches("0x").to_owned()
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError> {
		let did = Did::parse_snap(self.credential_subject.id.clone())?;
		let mut bytes = Vec::new();
		bytes.push(did.schema.into());
		bytes.extend_from_slice(&did.key);
		bytes.push(self.credential_subject.current_status.clone().into());

		Ok(bytes)
	}
}

impl IntoTerm for StatusSchema {
	fn into_term(self, timestamp: u64) -> Result<Vec<Term>, AttTrError> {
		// TODO: uncomment when verification spec is defined
		// let pk = self.validate()?;
		// let from_address = address_from_ecdsa_key(&pk);
		// let from_did: String = Did::new(Schema::PkhEth, from_address).into();
		// if from_did != self.issuer {
		// 	return Err(AttTrError::VerificationError);
		// }

		let weight = 50.;
		let domain = Domain::SoftwareSecurity;
		let form = match self.credential_subject.current_status {
			CurrentStatus::Endorsed => TermForm::Trust,
			CurrentStatus::Disputed => TermForm::Distrust,
		};

		let term = Term::new(
			self.issuer,
			self.credential_subject.id,
			weight,
			domain.into(),
			form,
			timestamp,
		);
		Ok(vec![term])
	}
}

#[cfg(test)]
mod test {
	use secp256k1::rand::thread_rng;
	use secp256k1::{generate_keypair, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	use mm_spd_vc::OneOrMore;

	use crate::did::Did;
	use crate::schemas::{Proof, Validation};
	use crate::utils::address_from_ecdsa_key;

	use super::*;

	#[test]
	fn should_validate_endorse_credential() {
		let did_string = "snap://0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let did = Did::parse_snap(did_string.clone()).unwrap();
		let current_status = CurrentStatus::Endorsed;

		let mut keccak = Keccak256::default();
		keccak.update([did.schema.into()]);
		keccak.update(&did.key);
		keccak.update([current_status.clone().into()]);
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

		let kind = OneOrMore::One("AuditReportDisapproveCredential".to_string());
		let address = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:0x{}", hex::encode(address));
		let cs = CredentialSubject::new(did_string, current_status, None);
		let proof = Proof { signature: Some(sig_string) };

		let follow_schema = StatusSchema { kind, issuer, credential_subject: cs, proof };

		let rec_pk = follow_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
