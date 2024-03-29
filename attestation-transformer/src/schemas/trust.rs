use serde_derive::{Deserialize, Serialize};

use crate::did::{Did, Schema};
use crate::error::AttTrError;
use crate::schemas::{Domain, IntoTerm, Proof, Validation};
use crate::term::Term;
use crate::utils::address_from_ecdsa_key;

#[derive(Deserialize, Serialize, Clone)]
pub struct DomainTrust {
	pub(crate) scope: Domain,
	pub(crate) level: f32,
	reason: Vec<String>,
}

impl DomainTrust {
	pub fn new(scope: Domain, level: f32, reason: Vec<String>) -> Self {
		Self { scope, level, reason }
	}
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CredentialSubject {
	id: String,
	trustworthiness: Vec<DomainTrust>,
}

impl CredentialSubject {
	pub fn new(id: String, trustworthiness: Vec<DomainTrust>) -> Self {
		Self { id, trustworthiness }
	}
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrustSchema {
	#[serde(alias = "type")]
	kind: String,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

impl TrustSchema {
	pub fn new(
		kind: String, issuer: String, credential_subject: CredentialSubject, proof: Proof,
	) -> Self {
		Self { kind, issuer, credential_subject, proof }
	}
}

impl Validation for TrustSchema {
	fn get_trimmed_signature(&self) -> String {
		self.proof.get_signature().trim_start_matches("0x").to_owned()
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError> {
		let did = Did::parse_pkh_eth(self.credential_subject.id.clone())?;

		let mut bytes = Vec::new();
		bytes.push(did.schema.into());
		bytes.extend_from_slice(&did.key);
		for arc in &self.credential_subject.trustworthiness {
			bytes.push(arc.scope.clone().into());
			// TODO: Uncomment when supported
			// bytes.extend_from_slice(&arc.level.to_be_bytes());
		}

		Ok(bytes)
	}
}

impl IntoTerm for TrustSchema {
	fn into_term(self, timestamp: u64) -> Result<Vec<Term>, AttTrError> {
		let pk = self.validate()?;

		let from_address = address_from_ecdsa_key(&pk);
		let from_did: String = Did::new(Schema::PkhEth, from_address).into();
		if from_did != self.issuer {
			return Err(AttTrError::VerificationError);
		}

		let mut terms = Vec::new();
		for trust_arc in &self.credential_subject.trustworthiness {
			let form = trust_arc.level >= 0.;
			let term_group = match trust_arc.scope {
				Domain::SoftwareDevelopment => vec![Term::new(
					from_did.clone(),
					self.credential_subject.id.clone(),
					trust_arc.level.abs() * 10.,
					Domain::SoftwareDevelopment.into(),
					form,
					timestamp,
				)],
				Domain::SoftwareSecurity => {
					vec![Term::new(
						from_did.clone(),
						self.credential_subject.id.clone(),
						trust_arc.level.abs() * 10.,
						Domain::SoftwareSecurity.into(),
						form,
						timestamp,
					)]
				},
				Domain::Honesty => {
					vec![
						Term::new(
							from_did.clone(),
							self.credential_subject.id.clone(),
							trust_arc.level.abs() * 1.,
							Domain::SoftwareDevelopment.into(),
							form,
							timestamp,
						),
						Term::new(
							from_did.clone(),
							self.credential_subject.id.clone(),
							trust_arc.level.abs() * 1.,
							Domain::SoftwareSecurity.into(),
							form,
							timestamp,
						),
					]
				},
			};

			terms.extend(term_group);
		}

		Ok(terms)
	}
}

#[cfg(test)]
mod test {
	use secp256k1::rand::thread_rng;
	use secp256k1::{generate_keypair, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	use crate::did::Did;
	use crate::schemas::{Domain, Proof, Validation};
	use crate::utils::address_from_ecdsa_key;

	use super::*;

	#[test]
	fn should_validate_trust_schema() {
		let did_string = "did:pkh:eth:0x90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let did = Did::parse_pkh_eth(did_string.clone()).unwrap();
		let trust_arc = DomainTrust::new(Domain::SoftwareSecurity, 0.5, Vec::new());

		let mut keccak = Keccak256::default();
		keccak.update([did.schema.into()]);
		keccak.update(&did.key);
		keccak.update([trust_arc.scope.clone().into()]);
		// keccak.update(&trust_arc.level.to_be_bytes());

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
		let addr = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:0x{}", hex::encode(addr));
		let cs = CredentialSubject { id: did_string, trustworthiness: vec![trust_arc] };
		let proof = Proof { signature: sig_string };

		let aa_schema = TrustSchema { kind, issuer, credential_subject: cs, proof };

		let rec_pk = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
