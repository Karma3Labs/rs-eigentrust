use hex::FromHexError;
use rocksdb::Error as RocksDbError;
use secp256k1::Error as SecpError;
use serde_json::Error;
use thiserror::Error;
use tonic::Status;

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

	#[error("ParseError")]
	ParseError,

	#[error("NotImplemented")]
	NotImplemented,
}

impl AttTrError {
	pub fn into_status(self) -> Status {
		Status::internal(format!("Internal error: {}", self))
	}
}
