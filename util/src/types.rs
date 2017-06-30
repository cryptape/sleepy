use parking_lot::RwLock;

use bigint::hash::{H256, H512};
use std::collections::{BTreeMap, HashMap, HashSet};
use bincode::{serialize, deserialize, Infinite};
use rand::{thread_rng, Rng};
use sha3::Hashable;

#[derive(Debug)]
pub enum Error {
    InvalidHeight,
    MissParent,
}

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
    inner: RwLock<ChainImpl>,
}

#[derive(Debug)]
struct ChainImpl {
    blocks: HashMap<H256, Block>,
    forks: BTreeMap<u64, Vec<H256>>,
    current_height: u64,
    current_hash: H256,
}

//select main
impl Chain {
    pub fn init() -> Self {
        Chain {
            inner: RwLock::new(ChainImpl {
                                   blocks: HashMap::new(),
                                   forks: BTreeMap::new(),
                                   current_height: 0,
                                   current_hash: H256::zero(),
                               }),
        }
    }

    pub fn insert(&self, block: Block) -> Result<(), Error> {
        let encoded: Vec<u8> = serialize(&block, Infinite).unwrap();
        let hash = encoded.sha3();
        let bh = block.height;
        let mut guard = self.inner.write();

        if guard.blocks.contains_key(&hash) {
            return Ok(());
        }

        if bh != guard.current_height + 1 {
            return Err(Error::InvalidHeight);
        }

        guard.blocks.insert(hash, block);

        guard.current_height = bh;
        let forks = {
            let forks = guard.forks.entry(bh).or_insert_with(Vec::new);
            forks.push(hash);
            forks.clone()
        };
        if forks.len() > 1 {
            let mut rng = thread_rng();
            let n: usize = rng.gen_range(0, forks.len());
            let pick = forks[n];
            guard.current_hash = pick;
        }
        Ok(())
    }

    pub fn get_status(&self) -> (u64, H256) {
        let guard = self.inner.read();
        (guard.current_height, guard.current_hash)
    }
}
