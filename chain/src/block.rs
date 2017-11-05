use util::*;
use crypto::{recover, Signature, sign};
use std::ops::{Deref, DerefMut};
use std::cell::Cell;
use std::cmp;
use error::*;
use transaction::SignedTransaction;
use bls;
use rlp::*;
use bytes::Bytes;

pub type BlockNumber = u64;
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Eq)]
pub struct HashWrap(Cell<Option<H256>>);

unsafe impl Sync for HashWrap {}

impl Deref for HashWrap {
    type Target = Cell<Option<H256>>;

    fn deref(&self) -> &Cell<Option<H256>> {
        &self.0
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Eq, Debug)]
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

    /// verify the proof.
	pub fn verify_proof(&self, anc_hash: H256, pubkey: Vec<u8>, g: Vec<u8>) -> bool {
        let sig = self.proof.time_signature.clone();
        let mut h1 = H256::from(self.timestamp).to_vec();
        let mut h2 = H256::from(self.height).to_vec();
        let mut h3 = anc_hash.to_vec();
        h1.append(&mut h2);
        h1.append(&mut h3);
        let hash = h1.sha3();
        bls::verify(hash.to_vec(), sig, pubkey, g)        
	}

    /// Get difficulty
    pub fn difficulty(&self) -> U256 {
        self.proof.time_signature.sha3().into()
    }

    /// Get the hash of this header.
    pub fn hash(&self) -> H256 {
        let hash = self.hash.get();
        match hash {
            Some(h) => h,
            None => {
                let h = self.rlp_hash();
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

    /// Place this header into an RLP stream `s`.
    pub fn stream_rlp(&self, s: &mut RlpStream) {
        s.begin_list(7);
        s.append(&self.parent_hash);
        s.append(&self.timestamp);
        s.append(&self.height);
        s.append(&self.transactions_root);
        s.append(&self.state_root);
        s.append(&self.receipts_root);
        s.append(&self.proof);

    }

    /// Get the RLP of this header.
    pub fn rlp(&self) -> Bytes {
        let mut s = RlpStream::new();
        self.stream_rlp(&mut s);
        s.out()
    }

    /// Get the hash (Keccak) of this header.
    pub fn rlp_hash(&self) -> H256 {
        self.rlp().sha3()
    }
}

impl Decodable for Header {
    fn decode(r: &UntrustedRlp) -> Result<Self, DecoderError> {
        let blockheader = Header {
            parent_hash: r.val_at(0)?,
            timestamp: cmp::min(r.val_at::<U256>(1)?, u64::max_value().into()).as_u64(),
            height: r.val_at(2)?,
            transactions_root: r.val_at(3)?,
            state_root: r.val_at(4)?,
            receipts_root: r.val_at(5)?,
            proof: r.val_at(6)?,
            hash: HashWrap(Cell::new(Some(r.as_raw().sha3()))),
        };

        Ok(blockheader)
    }
}

impl Encodable for Header {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.stream_rlp(s);
    }
}

impl HeapSizeOf for Header {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

#[derive(Clone, PartialEq, Eq, Debug, RlpEncodable, RlpDecodable)]
pub struct RichHeader {
    pub header: Header,
    pub verified: bool,
}

impl Deref for RichHeader {
    type Target = Header;

    fn deref(&self) -> &Header {
        &self.header
    }
}

impl DerefMut for RichHeader {
    fn deref_mut(&mut self) -> &mut Header {
        &mut self.header
    }
}

impl HeapSizeOf for RichHeader {
    fn heap_size_of_children(&self) -> usize {
        self.header.heap_size_of_children()
    }
}

#[derive(Hash, Serialize, Deserialize, Clone, PartialEq, Eq, Debug, RlpEncodable, RlpDecodable)]
pub struct Body {
    /// transactions
    pub transactions: Vec<SignedTransaction>
}

impl HeapSizeOf for Body {
    fn heap_size_of_children(&self) -> usize {
        self.transactions.heap_size_of_children()
    }
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

#[derive(Hash, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, RlpEncodable, RlpDecodable)]
pub struct Proof {
    pub time_signature: Vec<u8>,
    pub block_signature: H520,
}

impl Default for Proof {
	fn default() -> Self {
		Proof {
            time_signature: Vec::new(),
            block_signature: H520::default(),
		}
	}
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
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
               time_signature: Vec<u8>)
               -> Block {

        let proof = Proof {
            time_signature: time_signature,
            block_signature: H520::default(),
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
        let sig: Signature = self.proof.block_signature.into();
		recover(&sig, &self.hash()).map_err(|_| Error::InvalidSignature)
	}

    /// Generate the genesis block.
    pub fn genesis(timestamp: u64) -> Block {
        let mut block = Block::new();
        block.timestamp = timestamp;
        block
    }

    /// generate proof
    pub fn gen_proof(private_key: Vec<u8>, time: u64, height: u64, anc_hash: H256 ) -> Vec<u8> {
        let mut h1 = H256::from(time).to_vec();
        let mut h2 = H256::from(height).to_vec();
        let mut h3 = anc_hash.to_vec();
        h1.append(&mut h2);
        h1.append(&mut h3);
        let hash = h1.sha3();
        bls::sign(hash.to_vec(), private_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::KeyPair;

    #[test]
    fn test_proof_public() {
        let (private_key, public_key, g) = bls::key_gen();
        let parent_hash = H256::default();
        let timestamp = 12345;
        let proof = Block::gen_proof(private_key, timestamp, 1, H256::default());
        let block = Block::init(1, timestamp, parent_hash, Vec::new(), proof);
        assert_eq!(block.verify_proof(H256::default(), public_key, g), true);
    }

    #[test]
    fn test_sign_public() {
        let parent_hash = H256::default();
        let timestamp = 12345;
        let mut block = Block::init(1, timestamp, parent_hash, Vec::new(), Vec::new());
        let private_key = H256::from("40f2d8f8e1594579824fd04edfc7ff1ddffd6be153b23f4318e1acff037d3ea9",);
        let keypair = KeyPair::from_privkey(private_key).unwrap();
        block.sign(&private_key);
        assert_eq!(block.sign_public().unwrap(), *keypair.pubkey());
    }
}
