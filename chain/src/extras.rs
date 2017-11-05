// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Blockchain DB extras.

use std::ops;
// use std::io::Write;
use db::Key;
use block::{BlockNumber, RichHeader, Header, Body};

use heapsize::HeapSizeOf;
use bigint::hash::{H256, H264};
// use kvdb::PREFIX_LEN as DB_PREFIX_LEN;

/// Represents index of extra data in database
#[derive(Copy, Debug, Hash, Eq, PartialEq, Clone)]
pub enum ExtrasIndex {
	/// Block hash index
	BlockHash = 1,
	/// Transaction address index
	TransactionAddress = 2,
}

fn with_index(hash: &H256, i: ExtrasIndex) -> H264 {
	let mut result = H264::default();
	result[0] = i as u8;
	(*result)[1..].clone_from_slice(hash);
	result
}

pub struct BlockNumberKey([u8; 5]);

impl ops::Deref for BlockNumberKey {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl Key<H256> for BlockNumber {
	type Target = BlockNumberKey;

	fn key(&self) -> Self::Target {
		let mut result = [0u8; 5];
		result[0] = ExtrasIndex::BlockHash as u8;
		result[1] = (self >> 24) as u8;
		result[2] = (self >> 16) as u8;
		result[3] = (self >> 8) as u8;
		result[4] = *self as u8;
		BlockNumberKey(result)
	}
}

impl Key<Header> for H256 {
	type Target = H256;

	fn key(&self) -> H256 {
		*self
	}
}

impl Key<RichHeader> for H256 {
	type Target = H256;

	fn key(&self) -> H256 {
		*self
	}
}

impl Key<Body> for H256 {
	type Target = H256;

	fn key(&self) -> H256 {
		*self
	}
}

impl Key<TransactionAddress> for H256 {
	type Target = H264;

	fn key(&self) -> H264 {
		with_index(self, ExtrasIndex::TransactionAddress)
	}
}

/// Represents address of certain transaction within block
#[derive(Debug, PartialEq, Clone, RlpEncodable, RlpDecodable)]
pub struct TransactionAddress {
	/// Block hash
	pub block_hash: H256,
	/// Transaction index within the block
	pub index: usize
}

impl HeapSizeOf for TransactionAddress {
	fn heap_size_of_children(&self) -> usize { 0 }
}
