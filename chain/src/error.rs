use crypto::Error as CryptoError;

#[derive(Debug)]
pub enum Error {
    FutureBlock,
    UnknownParent,
    DuplicateBlock,
    DuplicateTransaction,
    InvalidTimestamp,
    InvalidReceiptsRoot,
    InvalidStateRoot,
    InvalidTransactionsRoot,
    InvalidPublicKey,
    InvalidSignature,
    InvalidFormat,
    CryptoError(CryptoError),
}

impl From<CryptoError> for Error {
	fn from(err: CryptoError) -> Self {
		Error::CryptoError(err)
	}
}