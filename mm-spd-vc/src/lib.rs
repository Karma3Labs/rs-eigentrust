use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VerifiableCredential {
	#[serde(rename = "@context")]
	pub context: Option<Vec<String>>,
	pub id: Option<String>,
	#[serde(rename = "type")]
	pub type_: OneOrMore<String>,
	pub issuer: String,
	pub issuance_date: Option<String>,
	pub credential_subject: serde_json::Value,
	pub proof: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StatusCredential {
	#[serde(rename = "@context")]
	pub context: Option<Vec<String>>,
	pub id: Option<String>,
	#[serde(rename = "type")]
	pub type_: OneOrMore<String>,
	pub issuer: String,
	pub issuance_date: Option<String>,
	pub credential_subject: StatusCredentialSubject,
	pub proof: StatusCredentialProof,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StatusCredentialSubject {
	pub id: String,
	pub current_status: String,
	// TODO(ek): Re-enable when format stabilizes
	// pub status_reason: Option<OneOrMore<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StatusCredentialProof {
	// TODO(ek): Implement
	// pub signature: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustScoreCredential {
	#[serde(rename = "@context")]
	pub context: Vec<String>,
	pub id: String,
	#[serde(rename = "type")]
	pub type_: OneOrMore<String>,
	pub issuer: String,
	pub issuance_date: String,
	pub credential_subject: TrustScoreCredentialSubject,
	pub proof: TrustScoreCredentialProof,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustScoreCredentialSubject {
	pub id: String,
	pub trust_score_type: String,
	pub trust_score: TrustScore,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustScore {
	pub value: f64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub confidence: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub accuracy: Option<f64>,
	pub scope: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustScoreCredentialProof {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
	pub issuer: String,
	pub issuance_date: String,
	pub locations: Option<Vec<String>>,
	pub proof: ManifestProof,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProof {}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OneOrMore<T> {
	One(T),
	More(Vec<T>),
}

impl OneOrMore<String> {
	pub fn matches(&self, s: &str) -> bool {
		match self {
			Self::One(v) => v == s,
			Self::More(v) => v.contains(&s.to_string()),
		}
	}
}
