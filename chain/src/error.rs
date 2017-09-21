#[derive(Debug, PartialEq, Eq)]
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
    InvalidProofKey,
    InvalidProof,
    InvalidSignature,
    InvalidFormat,
    UnknownAncestor,
    NTPError,
}