use secp256k1::PublicKey;
use sha3::{digest::Digest, Keccak256};

pub fn address_from_ecdsa_key(pub_key: &PublicKey) -> Vec<u8> {
	let raw_pub_key = pub_key.serialize_uncompressed();
	debug_assert_eq!(raw_pub_key[0], 0x04);
	// Hash and get the last 20 bytes.
	let pub_key_hash = Keccak256::digest(&raw_pub_key[1..]);
	println!("full pk: {:?}", pub_key_hash);
	pub_key_hash[12..].to_vec()
}

#[cfg(test)]
mod test {
	use super::address_from_ecdsa_key;
	use secp256k1::PublicKey;
	use std::str::FromStr;

	#[test]
	fn should_recreate_address_and_did_from_pk() {
		let address = "90f8bf6a479f320ead074411a4b0e7944ea8c9c1";
		let pk = "04e68acfc0253a10620dff706b0a1b1f1f5833ea3beb3bde2250d5f271f3563606672ebc45e0b7ea2e816ecb70ca03137b1c9476eec63d4632e990020b7b6fba39";

		let pk = PublicKey::from_str(pk).unwrap();
		let rec_address = address_from_ecdsa_key(&pk);

		assert_eq!(address, hex::encode(rec_address));
	}
}
