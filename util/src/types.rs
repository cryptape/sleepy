use parking_lot::RwLock;
use hash::{H256, H512};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Transcation {}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Block {
    pub height: u64,
    pub timestamp: u64,
    pub pubkey: H256,
    pub signature: H512,
    pub transactions: Vec<Transcation>,
    pub pre_hash: H256,
}

#[derive(Debug)]
pub struct Chain {
    inner: RwLock<ChainInner>,
}

#[derive(Debug)]
struct ChainInner {
    block_map: HashMap<H256, Block>,
    height_index: BTreeMap<u64, H256>,
    current_height: u64,
    current_hash: H256,
}
