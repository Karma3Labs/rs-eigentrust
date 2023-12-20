use crate::{did::Did, error::AttTrError, term::Term, utils::address_from_ecdsa_key};
use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	generate_keypair,
	rand::thread_rng,
	Message, PublicKey, Secp256k1,
};
use serde_derive::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

use super::{IntoTerm, Validation};

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
pub struct EndorseCredential {
	id: Did,
	status_reason: CurrentStatus,
	sig: String,
}

#[cfg(test)]
impl EndorseCredential {
	pub fn new(id: String, status_reason: CurrentStatus) -> Self {
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
		let (rec_id, sig_bytes) = res.serialize_compact();
		let rec_id_i32 = rec_id.to_i32();

		let mut bytes = Vec::new();
		bytes.copy_from_slice(&sig_bytes);
		bytes.copy_from_slice(&rec_id_i32.to_be_bytes());
		let encoded_sig = hex::encode(bytes);

		EndorseCredential { id: did, status_reason, sig: encoded_sig }
	}
}

impl Validation for EndorseCredential {
	fn validate(&self) -> Result<(PublicKey, bool), AttTrError> {
		let mut keccak = Keccak256::default();
		keccak.update(&self.id.key);
		keccak.update(&[self.status_reason.clone().into()]);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref())
			.map_err(|x| AttTrError::VerificationError(x))?;

		let sig_bytes =
			hex::decode(self.sig.clone()).map_err(|_| AttTrError::SerialisationError)?;
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
		Ok((
			pk,
			secp.verify_ecdsa(&message, &signature.to_standard(), &pk).is_ok(),
		))
	}
}

impl IntoTerm for EndorseCredential {
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Result<Term, AttTrError> {
		let (pk, valid) = self.validate()?;
		assert!(valid);

		let from_address = address_from_ecdsa_key(&pk);
		let to_address = hex::encode(&self.id.key);
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
	use crate::schemas::status::EndorseCredential;
	use crate::schemas::Validation;
	use crate::{did::Did, schemas::status::CurrentStatus};
	use secp256k1::{generate_keypair, rand::thread_rng, Message, Secp256k1};
	use sha3::{Digest, Keccak256};

	#[test]
	fn should_validate_endorse_credential() {
		let did_string = "did:pkh:eth:90f8bf6a479f320ead074411a4b0e7944ea8c9c2".to_owned();
		let did = Did::parse(did_string).unwrap();
		let status_reason = CurrentStatus::Endorsed;

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

		let follow_schema = EndorseCredential { id: did, status_reason, sig: sig_string };

		let (rec_pk, valid) = follow_schema.validate().unwrap();

		assert_eq!(rec_pk, pk);
		assert!(valid);
	}
}
