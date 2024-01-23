use super::{Domain, IntoTerm, Proof, Validation};
use crate::{
	did::{Did, Schema},
	error::AttTrError,
	term::Term,
	utils::address_from_ecdsa_key,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub enum SecurityStatus {
	Unsecure,
	Secure,
}

impl Into<u8> for SecurityStatus {
	fn into(self) -> u8 {
		match self {
			Self::Unsecure => 0,
			Self::Secure => 1,
		}
	}
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SecurityFinding {
	criticality: f32,
	#[serde(alias = "type")]
	kind: Option<String>,
	description: Option<String>,
	lang: Option<String>,
}

impl SecurityFinding {
	pub fn new(
		criticality: f32, kind: Option<String>, description: Option<String>, lang: Option<String>,
	) -> Self {
		Self { criticality, kind, description, lang }
	}
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSubject {
	id: String,
	security_status: SecurityStatus,
	security_findings: Vec<SecurityFinding>,
}

impl CredentialSubject {
	pub fn new(
		id: String, security_status: SecurityStatus, security_findings: Vec<SecurityFinding>,
	) -> Self {
		Self { id, security_status, security_findings }
	}
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SecurityReportSchema {
	#[serde(alias = "type")]
	kind: String,
	issuer: String,
	credential_subject: CredentialSubject,
	proof: Proof,
}

impl SecurityReportSchema {
	pub fn new(
		kind: String, issuer: String, credential_subject: CredentialSubject, proof: Proof,
	) -> Self {
		Self { kind, issuer, credential_subject, proof }
	}
}

impl Validation for SecurityReportSchema {
	fn get_trimmed_signature(&self) -> String {
		self.proof.get_signature().trim_start_matches("0x").to_owned()
	}

	fn get_message(&self) -> Result<Vec<u8>, AttTrError> {
		let did = Did::parse_snap(self.credential_subject.id.clone())?;
		let mut bytes = Vec::new();
		bytes.extend_from_slice(&did.key);
		bytes.push(self.credential_subject.security_status.clone().into());
		for finding in &self.credential_subject.security_findings {
			bytes.extend(finding.criticality.to_be_bytes());
		}

		Ok(bytes)
	}
}

impl IntoTerm for SecurityReportSchema {
	fn into_term(self, timestamp: u64) -> Result<Vec<Term>, AttTrError> {
		let pk = self.validate()?;

		let from_address = address_from_ecdsa_key(&pk);
		let from_did: String = Did::new(Schema::PkhEth, from_address).into();
		if from_did != self.issuer {
			return Err(AttTrError::VerificationError);
		}

		let form = match self.credential_subject.security_status {
			SecurityStatus::Unsecure => false,
			SecurityStatus::Secure => true,
		};

		let weight = 50.;
		let mut terms = Vec::new();
		if form {
			let term = Term::new(
				from_did,
				self.credential_subject.id,
				weight,
				Domain::SoftwareSecurity.into(),
				form,
				timestamp,
			);
			terms.push(term);
		} else {
			for finding in &self.credential_subject.security_findings {
				let term = Term::new(
					from_did.clone(),
					self.credential_subject.id.clone(),
					finding.criticality * weight,
					Domain::SoftwareSecurity.into(),
					form,
					timestamp,
				);
				terms.push(term);
			}
		}

		Ok(terms)
	}
}

#[cfg(test)]
mod test {
	use crate::{
		did::Did,
		schemas::{
			security::{CredentialSubject, SecurityFinding, SecurityReportSchema, SecurityStatus},
			Proof, Validation,
		},
		utils::address_from_ecdsa_key,
	};

	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_validate_audit_report_schema() {
		let did_string = "snap://90f8bf6a47".to_owned();
		let did = Did::parse_snap(did_string.clone()).unwrap();
		let security_status = SecurityStatus::Unsecure;
		let finding = SecurityFinding::new(0.5, None, None, None);

		let mut keccak = Keccak256::default();
		keccak.update(&did.key);
		keccak.update(&[security_status.clone().into()]);
		keccak.update(&finding.criticality.to_be_bytes());
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

		let kind = "SecurityReportCredential".to_string();
		let address = address_from_ecdsa_key(&pk);
		let issuer = format!("did:pkh:eth:{}", hex::encode(address));
		let cs = CredentialSubject::new(did_string, security_status, vec![finding]);
		let proof = Proof::new(sig_string);

		let aa_schema = SecurityReportSchema::new(kind, issuer, cs, proof);
		let rec_pk = aa_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
	}
}
