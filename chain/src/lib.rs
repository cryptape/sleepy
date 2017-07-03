extern crate util;
extern crate crypto;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate parking_lot;
extern crate rand;
extern crate bigint;
#[macro_use]
extern crate log;
extern crate bincode;

use parking_lot::{Mutex, RwLock};
use bigint::hash::{H256, H520, H512};
use std::collections::{BTreeMap, HashMap, HashSet};
use bincode::{serialize, deserialize, Infinite};
use rand::{thread_rng, Rng};
use util::Hashable;
use util::timestamp_now;
use crypto::{KeyPair, sign as crypto_sign, verify_public as crypto_vefify};
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::sync::Arc;

#[derive(Debug, PartialEq)]
pub enum Error {
    FutureHeight,
    FutureTime,
    MissParent,
    Duplicate,
    Malformated,
}

#[derive(Hash, Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Transcation {}

#[derive(Hash, Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Proof {
    pub timestamp: u64,
    pub key: H512,
    pub signature: H520,
}

#[derive(Hash, Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Block {
    pub height: u64,
    pub proof: Proof,
    pub transactions: Vec<Transcation>,
    pub pre_hash: H256,
}

#[derive(Hash, Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct SignedBlock {
    pub block: Block,
    pub singer: H512,
    pub signature: H520,
}

impl Block {
    pub fn new(height: u64,
               timestamp: u64,
               pre_hash: H256,
               proof_key: H512,
               signature: H520)
               -> Block {
        let proof = Proof {
            timestamp: timestamp,
            key: proof_key,
            signature: signature,
        };
        Block {
            height: height,
            proof: proof,
            transactions: Vec::new(),
            pre_hash: pre_hash,
        }
    }

    pub fn sign(self, keypair: &KeyPair) -> SignedBlock {
        let encoded: Vec<u8> = serialize(&self, Infinite).unwrap();
        let sign_hash = encoded.sha3();
        let signature: H520 = crypto_sign(keypair.privkey(), &sign_hash).unwrap().into();

        SignedBlock {
            block: self,
            singer: *keypair.pubkey(),
            signature: signature,
        }
    }

    pub fn is_first(&self) -> Result<bool, Error> {
        if self.height == 1 {
            if self.pre_hash != H256::zero() {
                return Err(Error::Malformated);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl SignedBlock {
    pub fn verify(&self) -> bool {
        let encoded: Vec<u8> = serialize(&self.block, Infinite).unwrap();
        let sign_hash = encoded.sha3();
        match crypto_vefify(&self.singer, &self.signature.into(), &sign_hash) {
            Ok(ret) => ret,
            _ => false,
        }
    }
}

impl Proof {
    pub fn verify(&self) -> bool {
        match crypto_vefify(&self.key,
                            &self.signature.into(),
                            &H256::from(self.timestamp)) {
            Ok(ret) => ret,
            _ => false,
        }
    }
}

impl ::std::ops::Deref for SignedBlock {
    type Target = Block;

    #[inline]
    fn deref(&self) -> &Block {
        &self.block
    }
}

#[derive(Debug)]
pub struct Chain {
    inner: ChainImpl,
    sender: Mutex<Sender<u64>>,
}

#[derive(Debug)]
struct ChainImpl {
    blocks: RwLock<HashMap<H256, SignedBlock>>,
    timestamp_future: RwLock<BTreeMap<u64, HashSet<SignedBlock>>>,
    height_future: RwLock<BTreeMap<u64, HashSet<SignedBlock>>>,
    parent_future: RwLock<BTreeMap<H256, HashSet<SignedBlock>>>,
    forks: RwLock<BTreeMap<u64, Vec<H256>>>,
    main: RwLock<HashMap<u64, H256>>,
    current_height: RwLock<u64>,
    current_hash: RwLock<H256>,
}

//TODO maintenance longest chain
//fetch miss parent
impl Chain {
    pub fn init() -> Arc<Self> {
        let (sender, receiver) = channel();
        let chain = Arc::new(Chain {
                                 inner: ChainImpl {
                                     blocks: RwLock::new(HashMap::new()),
                                     timestamp_future: RwLock::new(BTreeMap::new()),
                                     height_future: RwLock::new(BTreeMap::new()),
                                     parent_future: RwLock::new(BTreeMap::new()),
                                     forks: RwLock::new(BTreeMap::new()),
                                     main: RwLock::new(HashMap::new()),
                                     current_height: RwLock::new(0),
                                     current_hash: RwLock::new(H256::zero()),
                                 },
                                 sender: Mutex::new(sender),
                             });
        chain
    }

    pub fn insert(&self, block: &SignedBlock) -> Result<(), Error> {
        let encoded: Vec<u8> = serialize(block, Infinite).unwrap();
        let hash = encoded.sha3();
        let bh = block.height;
        {
            let mut blocks = self.inner.blocks.write();
            let mut current_height = self.inner.current_height.write();
            let mut current_hash = self.inner.current_hash.write();
            let mut forks = self.inner.forks.write();
            let mut main = self.inner.main.write();

            if blocks.contains_key(&hash) {
                return Err(Error::Duplicate);
            }

            if bh > *current_height + 1 {
                let mut height_future = self.inner.height_future.write();
                let future = height_future.entry(bh).or_insert_with(HashSet::new);
                future.insert(block.clone());
                return Err(Error::FutureHeight);
            }

            if block.proof.timestamp > timestamp_now() {
                let mut timestamp_future = self.inner.timestamp_future.write();
                let future = timestamp_future
                    .entry(block.proof.timestamp)
                    .or_insert_with(HashSet::new);
                future.insert(block.clone());
                return Err(Error::FutureTime);
            }

            info!("blocks {:?}, parent {:?}", *blocks, &block.pre_hash);
            if !block.is_first()? && !blocks.contains_key(&block.pre_hash) {
                let mut parent_future = self.inner.parent_future.write();
                let future = parent_future
                    .entry(block.pre_hash)
                    .or_insert_with(HashSet::new);
                future.insert(block.clone());
                return Err(Error::MissParent);
            }

            if bh == *current_height + 1 {
                *current_height = bh;
                *current_hash = hash;
                main.insert(bh, hash);
                info!("insert a block {:?} {:?} {:?}",
                      bh,
                      hash,
                      block.proof.timestamp);
                let forks = forks.entry(bh).or_insert_with(Vec::new);
                forks.push(hash);
                // tmp impl:  rand pick a fork
                if forks.len() > 1 {
                    info!("we meet fork!");
                    let mut rng = thread_rng();
                    let n: usize = rng.gen_range(0, forks.len());
                    let pick = forks[n];
                    *current_hash = pick;
                    main.insert(bh, hash);

                    let mut start_bh = bh - 1;
                    if main.get(&start_bh) != Some(&block.pre_hash) {
                        main.insert(start_bh, block.pre_hash);
                        loop {
                            let block = blocks.get(&block.pre_hash).cloned().unwrap();
                            start_bh -= 1;
                            if main.get(&start_bh) == Some(&block.pre_hash) {
                                break;
                            }
                            main.insert(start_bh, block.pre_hash);
                        }
                    }
                }
            }
            blocks.insert(hash, block.clone());
        }

        let pendings = {
            let mut parent_future = self.inner.parent_future.write();
            if parent_future.contains_key(&hash) {
                parent_future.remove(&hash)
            } else {
                None
            }
        };

        pendings.map(|blks| for blk in blks {
                         let _ = self.insert(&blk);
                     });
        Ok(())
    }

    pub fn get_status(&self) -> (u64, H256) {
        let current_height = self.inner.current_height.read();
        let current_hash = self.inner.current_hash.read();
        (*current_height, *current_hash)
    }

    pub fn get_block_by_hash(&self, hash: &H256) -> Option<SignedBlock> {
        self.inner.blocks.read().get(hash).cloned()
    }
}
