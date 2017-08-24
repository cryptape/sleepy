use util::*;
use crypto::{recover, Signature, sign};
use bincode::{serialize, Infinite};
use std::ops::{Deref, DerefMut};
use std::cell::Cell;
use error::*;
use transaction::SignedTransaction;


#[derive(Debug, PartialEq, Clone, Eq)]
pub struct HashWrap(Cell<Option<H256>>);

unsafe impl Sync for HashWrap {}

impl Deref for HashWrap {
    type Target = Cell<Option<H256>>;

    fn deref(&self) -> &Cell<Option<H256>> {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Header {
    /// Parent hash.
	pub parent_hash: H256,
	/// Block timestamp.
	pub timestamp: u64,
	/// Block height.
	pub height: u64,
	/// Transactions root.
	pub transactions_root: H256,
	/// State root.
	pub state_root: H256,
	/// Block receipts root.
	pub receipts_root: H256,
    /// Block hash
    pub hash: HashWrap,
    /// Block proof
    pub proof: Proof,
}

impl Default for Header {
	fn default() -> Self {
		Header {
			parent_hash: H256::default(),
			timestamp: 0,
			height: 0,
			transactions_root: SHA3_NULL_RLP,
			state_root: SHA3_NULL_RLP,
			receipts_root: SHA3_NULL_RLP,
            hash: HashWrap(Cell::new(None)),
            proof: Proof::default(),
		}
	}
}

impl Header {
    pub fn new() -> Header {
        Self::default()
    }

    /// Recovers the public key of the proof.
	pub fn proof_public(&self) -> Result<H512, Error> {
		Ok(recover(&self.proof.time_signature, &H256::from(self.timestamp))?)
	}

    ///generate proof
    pub fn gen_proof(&self, private_key: &H256) -> Signature {
        sign(private_key, &H256::from(self.timestamp)).unwrap().into()
    }

    /// calculate the hash of the header
    pub fn cal_hash(&self) -> H256 {
        let binwrap = (self.parent_hash, self.timestamp, self.height, self.transactions_root, 
                       self.state_root, self.receipts_root, self.proof.time_signature.to_vec());
        serialize(&binwrap, Infinite).unwrap().sha3()
    }

    /// Get the hash of this header.
    pub fn hash(&self) -> H256 {
        let hash = self.hash.get();
        match hash {
            Some(h) => h,
            None => {
                let h = self.cal_hash();
                self.hash.set(Some(h.clone()));
                h
            }
        }
    }

    ///if is genesis
    pub fn is_genesis(&self) -> Result<bool, Error> {
        if self.height == 0 {
            if self.parent_hash != H256::default() {
                return Err(Error::InvalidFormat);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Debug)]
pub struct Body {
    /// transactions
    pub transactions: Vec<SignedTransaction>
}

impl Default for Body {
	fn default() -> Self {
		Body {
            transactions: Vec::new(),
		}
	}
}

impl Body {
    ///calculate the transaction root
    pub fn transactions_root(&self) -> H256 {
        complete_merkle_root_raw(self.transactions.iter().map(|r| r.hash()).collect())
    }
}

#[derive(Hash, Clone, PartialEq, Eq, Debug)]
pub struct Proof {
    pub time_signature: Signature,
    pub block_signature: Signature,
}

impl Default for Proof {
	fn default() -> Self {
		Proof {
            time_signature: Signature::default(),
            block_signature: Signature::default(),
		}
	}
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Block {
    pub header: Header,
    pub body: Body,
}

impl Default for Block {
	fn default() -> Self {
		Block {
            header: Header::default(),
            body: Body::default(),
		}
	}
}

impl Deref for Block {
    type Target = Header;

    fn deref(&self) -> &Header {
        &self.header
    }
}

impl DerefMut for Block {
    fn deref_mut(&mut self) -> &mut Header {
        &mut self.header
    }
}

impl Block {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn init(height: u64,
               timestamp: u64,
               parent_hash: H256,
               transactions: Vec<SignedTransaction>,
               time_signature: Signature)
               -> Block {

        let proof = Proof {
            time_signature: time_signature,
            block_signature: Signature::default(),
        };

        let header = Header {
			parent_hash: parent_hash,
			timestamp: timestamp,
			height: height,
			transactions_root: SHA3_NULL_RLP,
			state_root: SHA3_NULL_RLP,
			receipts_root: SHA3_NULL_RLP,
            hash: HashWrap(Cell::new(None)),
            proof: proof,
		};

        let body = Body {
            transactions: transactions,
        };

        Block {
            header: header,
            body: body,
        }
    }

    ///sign block
    pub fn sign(&mut self, private_key: &H256) {
        let signature = sign(private_key, &self.hash()).unwrap().into();
        self.proof.block_signature = signature;
    }

    /// Recovers the public key of the signer.
	pub fn sign_public(&self) -> Result<H512, Error> {
		Ok(recover(&self.proof.block_signature, &self.hash())?)
	}

    /// Generate the genesis block.
    pub fn genesis(timestamp: u64) -> Block {
        let mut block = Block::new();
        block.timestamp = timestamp;
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::KeyPair;

    #[test]
    fn test_proof_public() {
        let private_key = H256::from("40f2d8f8e1594579824fd04edfc7ff1ddffd6be153b23f4318e1acff037d3ea9",);
        let keypair = KeyPair::from_privkey(private_key).unwrap();
        let parent_hash = H256::default();
        let timestamp = 12345;
        let sig = sign(keypair.privkey().into(), &H256::from(timestamp)).unwrap();
        let block = Block::init(1, timestamp, parent_hash, Vec::new(), sig.into());
        assert_eq!(block.proof_public().unwrap(), *keypair.pubkey());
    }

    #[test]
    fn test_sign_public() {
        let parent_hash = H256::default();
        let timestamp = 12345;
        let mut block = Block::init(1, timestamp, parent_hash, Vec::new(), Signature::default());
        let private_key = H256::from("40f2d8f8e1594579824fd04edfc7ff1ddffd6be153b23f4318e1acff037d3ea9",);
        let keypair = KeyPair::from_privkey(private_key).unwrap();
        block.sign(&private_key);
        assert_eq!(block.sign_public().unwrap(), *keypair.pubkey());
    }
}
