use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusCredential {
	#[serde(rename = "@context")]
	pub context: Option<Vec<String>>,
	pub id: Option<String>,
	#[serde(rename = "type")]
	pub type_: String,
	pub issuer: String,
	#[serde(rename = "issuanceDate")]
	pub issuance_date: Option<String>,
	#[serde(rename = "credentialSubject")]
	pub credential_subject: StatusCredentialSubject,
	pub proof: StatusCredentialProof,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusCredentialSubject {
	pub id: String,
	#[serde(rename = "currentStatus")]
	pub current_status: String,
	#[serde(rename = "statusReason")]
	pub status_reason: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusCredentialProof {
	pub signature: String,
}

#[derive(Serialize, Deserialize)]
pub struct TrustScoreCredential {
	#[serde(rename = "@context")]
	pub context: Vec<String>,
	pub id: String,
	#[serde(rename = "type")]
	pub type_: Vec<String>,
	pub issuer: String,
	#[serde(rename = "issuanceDate")]
	pub issuance_date: String,
	#[serde(rename = "credentialSubject")]
	pub credential_subject: TrustScoreCredentialSubject,
	pub proof: TrustScoreCredentialProof,
}

#[derive(Serialize, Deserialize)]
pub struct TrustScoreCredentialSubject {
	pub id: String,
	#[serde(rename = "trustScoreType")]
	pub trust_score_type: String,
	#[serde(rename = "trustScore")]
	pub trust_score: TrustScore,
}

#[derive(Serialize, Deserialize)]
pub struct TrustScore {
	pub value: f64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub confidence: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct TrustScoreCredentialProof {}

#[derive(Serialize, Deserialize)]
pub struct Manifest {
	pub issuer: String,
	#[serde(rename = "issuanceDate")]
	pub issuance_date: String,
	pub locations: Option<Vec<String>>,
	pub proof: ManifestProof,
}

#[derive(Serialize, Deserialize)]
pub struct ManifestProof {}
