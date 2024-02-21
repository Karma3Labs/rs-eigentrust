use itertools::Itertools;

#[derive(Debug, thiserror::Error)]
pub enum CanonicalizePeerDidError {
	#[error("unrecognized PKH namespace {0:?}")]
	BadPkhNamespace(String),
	#[error("unrecognized PKH method-specific ID {0:?}")]
	BadPkhMsid(String),
	#[error("unrecognized DID method {0:?}")]
	BadDidMethod(String),
	#[error("not a DID")]
	NotDid,
}

/// Canonicalize a peer DID, by lowercasing its 0x address portion and substituting chain ID 1.
/// Also convert legacy ether DIDs into pkh:eip155 DIDs.
pub fn canonicalize_peer_did(did: &str) -> Result<String, CanonicalizePeerDidError> {
	if let Some((scheme, method, msid)) = did.splitn(3, ':').collect_tuple() {
		if scheme == "did" {
			match method {
				"pkh" => {
					let fields = msid.split(':');
					if let Some(("eth", address)) = fields.to_owned().collect_tuple() {
						Ok(format!("did:pkh:eip155:1:{}", address.to_lowercase()))
					} else if let Some((namespace, _chain_id, address)) =
						fields.to_owned().collect_tuple()
					{
						match namespace {
							"eip155" => Ok(format!("did:pkh:eip155:1:{}", address.to_lowercase())),
							_ => Err(CanonicalizePeerDidError::BadPkhNamespace(namespace.into())),
						}
					} else {
						Err(CanonicalizePeerDidError::BadPkhMsid(msid.into()))
					}
				},
				"eth" => Ok(format!("did:pkh:eip155:1:{}", msid.to_lowercase())),
				_ => Err(CanonicalizePeerDidError::BadDidMethod(method.into())),
			}
		} else {
			Err(CanonicalizePeerDidError::NotDid)
		}
	} else {
		Err(CanonicalizePeerDidError::NotDid)
	}
}

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_canonicalize_did_non_did_scheme() {
		assert_matches!(
			canonicalize_peer_did("SCHEME:EXTRA"),
			Err(CanonicalizePeerDidError::NotDid)
		);
	}

	#[test]
	fn test_canonicalize_did_no_msid() {
		assert_matches!(
			canonicalize_peer_did("did:METHOD"),
			Err(CanonicalizePeerDidError::NotDid)
		);
	}

	#[test]
	fn test_canonicalize_did_non_pkh() {
		assert_matches!(
			canonicalize_peer_did("did:METHOD:MSID"),
			Err(CanonicalizePeerDidError::BadDidMethod(method)) if method == "METHOD"
		);
	}

	#[test]
	fn test_canonicalize_did_no_pkh_namespace() {
		assert_matches!(
			canonicalize_peer_did("did:pkh"),
			Err(CanonicalizePeerDidError::NotDid)
		);
	}

	#[test]
	fn test_canonicalize_did_no_pkh_chain_id() {
		assert_matches!(
			canonicalize_peer_did("did:pkh:NAMESPACE"),
			Err(CanonicalizePeerDidError::BadPkhMsid(msid)) if msid == "NAMESPACE"
		);
	}

	#[test]
	fn test_canonicalize_did_no_pkh_account_id() {
		assert_matches!(
			canonicalize_peer_did("did:pkh:NAMESPACE:CHAIN-ID"),
			Err(CanonicalizePeerDidError::BadPkhMsid(msid)) if msid == "NAMESPACE:CHAIN-ID"
		);
	}

	#[test]
	fn test_canonicalize_did_non_eip155() {
		assert_matches!(
			canonicalize_peer_did("did:pkh:NAMESPACE:CHAIN-ID:ACCOUNT-ID"),
			Err(CanonicalizePeerDidError::BadPkhNamespace(ns)) if ns == "NAMESPACE"
		);
	}

	#[test]
	fn test_canonicalize_did_pkh_eip155() {
		assert_matches!(
			canonicalize_peer_did("did:pkh:eip155:135:0x0123456789ABCDEF0123456789ABCDEF01234567"),
			Ok(did) if did == "did:pkh:eip155:1:0x0123456789abcdef0123456789abcdef01234567"
		);
	}

	#[test]
	fn test_canonicalize_did_eth() {
		assert_matches!(
			canonicalize_peer_did("did:eth:0x0123456789ABCDEF0123456789ABCDEF01234567"),
			Ok(did) if did == "did:pkh:eip155:1:0x0123456789abcdef0123456789abcdef01234567"
		);
	}

	#[test]
	fn test_canonicalize_did_pkh_eth() {
		assert_matches!(
			canonicalize_peer_did("did:pkh:eth:0x0123456789ABCDEF0123456789ABCDEF01234567"),
			Ok(did) if did == "did:pkh:eip155:1:0x0123456789abcdef0123456789abcdef01234567"
		);
	}
}
