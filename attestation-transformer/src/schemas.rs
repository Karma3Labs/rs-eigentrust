use secp256k1::{
	ecdsa::{RecoverableSignature, RecoveryId},
	Message, PublicKey, Secp256k1,
};
use serde_derive::Deserialize;
use sha3::{digest::Digest, Keccak256};

use crate::term::{IntoTerm, Term};

use super::term::Validation;

#[derive(Deserialize, Clone)]
pub enum Scope {
	Reviewer,
	Developer,
	Auditor,
}

impl Into<u8> for Scope {
	fn into(self) -> u8 {
		match self {
			Self::Reviewer => 0,
			Self::Developer => 1,
			Self::Auditor => 2,
		}
	}
}

#[derive(Deserialize)]
pub struct FollowSchema {
	id: String,
	is_trustworthy: bool,
	scope: Scope,
	sig: (i32, [u8; 32], [u8; 32]),
}

impl Validation for FollowSchema {
	fn validate(&self) -> (PublicKey, bool) {
		let mut keccak = Keccak256::default();
		keccak.update(self.id.as_bytes());
		keccak.update(&[self.is_trustworthy.into()]);
		keccak.update(&[self.scope.clone().into()]);
		let digest = keccak.finalize();
		let message = Message::from_digest_slice(digest.as_ref()).unwrap();

		let mut rs_bytes = [0; 64];
		rs_bytes[..32].copy_from_slice(&self.sig.1);
		rs_bytes[32..].copy_from_slice(&self.sig.2);
		let signature = RecoverableSignature::from_compact(
			&rs_bytes,
			RecoveryId::from_i32(self.sig.0).unwrap(),
		)
		.unwrap();
		let pk = signature.recover(&message).unwrap();

		let secp = Secp256k1::verification_only();
		(
			pk,
			secp.verify_ecdsa(&message, &signature.to_standard(), &pk).is_ok(),
		)
	}
}

impl IntoTerm for FollowSchema {
	const WEIGHT: u32 = 50;
	const DOMAIN: u32 = 1;

	fn into_term(self) -> Term {
		let (pk, valid) = self.validate();
		assert!(valid);

		let address = address_from_ecdsa_key(&pk);
		let sender_did = address_to_did(&address);

		Term::new(sender_did, self.id, Self::WEIGHT, Self::DOMAIN)
	}
}

pub fn address_from_ecdsa_key(pub_key: &PublicKey) -> String {
	let raw_pub_key = pub_key.serialize_uncompressed();
	let (x, y) = raw_pub_key.split_at(32);

	// Reverse and concatenate x and y coordinates.
	let rev_x: Vec<u8> = x.iter().rev().cloned().collect();
	let rev_y: Vec<u8> = y.iter().rev().cloned().collect();
	let pub_key = [rev_x, rev_y].concat();

	// Hash and get the last 20 bytes.
	let pub_key_hash = Keccak256::digest(pub_key);
	let address: &[u8] = &pub_key_hash[pub_key_hash.len() - 20..];

	// Get little endian address
	let le_address: Vec<u8> = address.iter().rev().cloned().collect();
	let address = hex::encode(&le_address);

	address
}

pub fn address_to_did(address: &str) -> String {
	let mut did = "did:eth:".to_string();
	did.push_str(address);

	did
}
