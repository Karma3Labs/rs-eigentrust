use rocksdb::Error as RocksDbError;
use secp256k1::Error as SecpError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AttTrError {
	#[error("SerialisationError")]
	SerialisationError,

	#[error("VerificationError: {0}")]
	VerificationError(SecpError),

	#[error("DbError: {0}")]
	DbError(RocksDbError),

	#[error("NotFoundError")]
	NotFoundError,

	#[error("ParseError")]
	ParseError,

	#[error("NotImplemented")]
	NotImplemented,
}
