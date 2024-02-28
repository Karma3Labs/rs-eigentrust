use rocksdb::Error as RocksDbError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LcError {
	#[error("SerialisationError")]
	SerialisationError,

	#[error("DbError: {0}")]
	DbError(RocksDbError),

	#[error("NotFoundError")]
	NotFoundError,

	#[error("ParseError")]
	ParseError,
}

impl From<LcError> for tonic::Status {
	fn from(value: LcError) -> Self {
		Self::internal(format!("Internal error: {}", value))
	}
}
