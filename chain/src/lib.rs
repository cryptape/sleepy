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

#[derive(Debug)]
pub enum Error {
    FutureHeight,
    FutureTime,
    MissParent,
    Duplicate,
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
            singer: keypair.pubkey().clone(),
            signature: signature,
        }
    }
}

impl SignedBlock {
    pub fn verify(&self) -> bool {
        let encoded: Vec<u8> = serialize(&self, Infinite).unwrap();
        let sign_hash = encoded.sha3();
        match crypto_vefify(&self.singer, &self.signature, &sign_hash) {
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
    inner: RwLock<ChainImpl>,
    sender: Mutex<Sender<u64>>,
}

#[derive(Debug)]
struct ChainImpl {
    blocks: HashMap<H256, SignedBlock>,
    timestamp_future: BTreeMap<u64, HashSet<SignedBlock>>,
    height_future: BTreeMap<u64, HashSet<SignedBlock>>,
    parent_future: BTreeMap<H256, HashSet<SignedBlock>>,
    forks: BTreeMap<u64, Vec<H256>>,
    main: HashMap<u64, SignedBlock>,
    current_height: u64,
    current_hash: H256,
}

//TODO maintenance longest chain
//fetch miss parent
impl Chain {
    pub fn init() -> Arc<Self> {
        let (sender, receiver) = channel();
        let chain = Arc::new(Chain {
                                 inner: RwLock::new(ChainImpl {
                                                        blocks: HashMap::new(),
                                                        timestamp_future: BTreeMap::new(),
                                                        height_future: BTreeMap::new(),
                                                        parent_future: BTreeMap::new(),
                                                        forks: BTreeMap::new(),
                                                        main: HashMap::new(),
                                                        current_height: 0,
                                                        current_hash: H256::zero(),
                                                    }),
                                 sender: Mutex::new(sender),
                             });
        let mario = chain.clone();
        thread::spawn(move || loop {
                          info!("mario maintenance!");
                          let height = receiver.recv().unwrap();
                          mario.maintenance(height);
                      });
        chain
    }

    pub fn insert(&self, block: &SignedBlock) -> Result<(), Error> {
        let encoded: Vec<u8> = serialize(block, Infinite).unwrap();
        let hash = encoded.sha3();
        let bh = block.height;
        let mut guard = self.inner.write();

        if guard.blocks.contains_key(&hash) {
            return Err(Error::Duplicate);
        }

        if bh > guard.current_height + 1 {
            let future = guard.height_future.entry(bh).or_insert_with(HashSet::new);
            future.insert(block.clone());
            return Err(Error::FutureHeight);
        }

        if block.proof.timestamp > timestamp_now() {
            let future = guard
                .timestamp_future
                .entry(block.proof.timestamp)
                .or_insert_with(HashSet::new);
            future.insert(block.clone());
            return Err(Error::FutureTime);
        }

        if !guard.blocks.contains_key(&block.pre_hash) {
            let future = guard
                .parent_future
                .entry(block.pre_hash)
                .or_insert_with(HashSet::new);
            future.insert(block.clone());
            return Err(Error::MissParent);
        }

        if bh == guard.current_height + 1 {
            guard.current_height = bh;
        }

        guard.blocks.insert(hash, block.clone());

        let forks = {
            let forks = guard.forks.entry(bh).or_insert_with(Vec::new);
            forks.push(hash);
            forks.clone()
        };

        // tmp impl:  rand pick a fork
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


    fn maintenance(&self, height: u64) {}
}
