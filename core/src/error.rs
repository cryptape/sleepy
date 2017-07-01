use std::fmt;
use util::error::*;
use bigint::hash::{H256, H520};
use bigint::uint::U256;

type BlockNumber = u64;

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
/// Errors concerning block processing.
pub enum BlockError {
    /// Public key is not found or invalid.
    InvalidPublicKey(H256),
    /// signature for the block is invalid.
    InvalidSignature(H520),
	/// State root header field is invalid.
	InvalidStateRoot(Mismatch<H256>),
	/// Transactions root header field is invalid.
	InvalidTransactionsRoot(Mismatch<H256>),
	/// Proof-of-work aspect of seal, which we assume is a 256-bit value, is invalid.
	InvalidProofOfWork(OutOfBounds<U256>),
	/// Receipts trie root header field is invalid.
	InvalidReceiptsRoot(Mismatch<H256>),
	/// Timestamp header field is invalid.
	InvalidTimestamp(OutOfBounds<u64>),
    /// Timestamp is later than current time
    BlockInFuture(OutOfBounds<u64>),
	/// Parent hash field of header is invalid; this is an invalid error indicating a logic flaw in the codebase.
	/// TODO: remove and favour an assert!/panic!.
	InvalidParentHash(Mismatch<H256>),
	/// Number field of header is invalid.
	InvalidNumber(Mismatch<BlockNumber>),
	/// Block number isn't sensible.
	RidiculousNumber(OutOfBounds<BlockNumber>),
    /// Parent given is unknown.
	UnknownParent(H256),
}

impl fmt::Display for BlockError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		use self::BlockError::*;

		let msg = match *self {
			InvalidStateRoot(ref mis) => format!("Invalid state root in header: {}", mis),
			InvalidTransactionsRoot(ref mis) => format!("Invalid transactions root in header: {}", mis),
			InvalidProofOfWork(ref oob) => format!("Block has invalid PoW: {}", oob),
			InvalidReceiptsRoot(ref mis) => format!("Invalid receipts trie root in header: {}", mis),
			InvalidTimestamp(ref oob) => format!("Invalid timestamp in header: {}", oob),
			InvalidParentHash(ref mis) => format!("Invalid parent hash: {}", mis),
			InvalidNumber(ref mis) => format!("Invalid number in header: {}", mis),
			RidiculousNumber(ref oob) => format!("Implausible block number. {}", oob),
			UnknownParent(ref hash) => format!("Unknown parent: {}", hash),
            InvalidPublicKey(ref pk) => format!("Invalid public key: {}", pk),
            InvalidSignature(ref sig) => format!("Invalid signature: {}", sig),
            BlockInFuture(ref oob) => format!("block in future: {}", oob),
 		};

		f.write_fmt(format_args!("Block error ({})", msg))
	}
}