use rocksdb::Error as RocksDbError;
use thiserror::Error;
pub use tonic::Status;

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

impl LcError {
	pub fn into_status(self) -> Status {
		Status::internal(format!("Internal error: {}", self))
	}
}
