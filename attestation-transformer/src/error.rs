use hex::FromHexError;
use mm_spd_did::CanonicalizePeerDidError;
use rocksdb::Error as RocksDbError;
use secp256k1::Error as SecpError;
use serde_json::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AttTrError {
	#[error("SerialisationError")]
	SerialisationError,

	#[error("SerdeError: {0}")]
	SerdeError(Error),

	#[error("HexError: {0}")]
	HexError(FromHexError),

	#[error("SigVerificationError: {0}")]
	SigVerificationError(SecpError),

	#[error("VerificationError")]
	VerificationError,

	#[error("DbError: {0}")]
	DbError(RocksDbError),

	#[error("NotFoundError")]
	NotFoundError,

	#[error("invalid subject or issuer DID: {0}")]
	InvalidPeerDid(CanonicalizePeerDidError),

	#[error("ParseError")]
	ParseError,
}

impl From<AttTrError> for tonic::Status {
	fn from(value: AttTrError) -> Self {
		Self::internal(format!("Internal error: {}", value))
	}
}
