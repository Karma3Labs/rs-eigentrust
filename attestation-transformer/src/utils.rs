use secp256k1::PublicKey;
use sha3::{digest::Digest, Keccak256};

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
