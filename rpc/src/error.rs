use jsonrpsee::{
	core::Error as JsonRpseeError,
	types::error::{CallError, ErrorObject},
};

mod error_codes {
	pub const RUNTIME_ERROR: i32 = 1; // Arbitrary number, but substrate uses the same
	pub const OFFCHAIN_INDEXING_DISABLED_ERROR: i32 = 2;
	pub const STORAGE_NOT_FOUND_ERROR: i32 = 3;
	pub const UNKNOWN_ERROR: i32 = 100;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Error while calling into the runtime: {0}")]
	Runtime(Box<dyn std::error::Error + Send + Sync>),
	#[error("This rpc is not allowed with offchain-indexing disabled: {0}")]
	OffchainIndexingDisabled(String),
	#[error("Offchain storage not found: {0}")]
	OffchainStorageNotFound(String),
	#[error("Other error: {0}")]
	Other(Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
	fn code(&self) -> i32 {
		use Error::*;
		match self {
			Runtime(_) => error_codes::RUNTIME_ERROR,
			OffchainIndexingDisabled(_) => error_codes::OFFCHAIN_INDEXING_DISABLED_ERROR,
			OffchainStorageNotFound(_) => error_codes::STORAGE_NOT_FOUND_ERROR,
			Other(_) => error_codes::UNKNOWN_ERROR,
		}
	}
}

impl From<Error> for JsonRpseeError {
	fn from(err: Error) -> Self {
		CallError::Custom(ErrorObject::owned(err.code(), err.to_string(), None::<()>)).into()
	}
}
