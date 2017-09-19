use parking_lot::{Mutex, RwLock};
use util::hash::H256;
use std::collections::{BTreeMap, HashMap, HashSet};
use rand::{thread_rng, Rng};
use util::config::SleepyConfig;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::sync::Arc;
use std::time::Duration;
use block::{Block, Body, Header};
use transaction::{SignedTransaction, TransactionAddress};
use error::*;


#[derive(Debug)]
pub struct Chain {
    block_headers: RwLock<HashMap<H256, Header>>,
    block_bodies: RwLock<HashMap<H256, Body>>,
    future_blocks: RwLock<Vec<Block>>,
    unknown_parent: RwLock<HashMap<H256, Vec<Block>>>,
    transaction_addresses: RwLock<HashMap<H256, TransactionAddress>>,
    block_hashes: RwLock<BTreeMap<u64,H256>>,
    current_height: RwLock<u64>,
    current_hash: RwLock<H256>,
    sender: Mutex<Sender<(u64, H256)>>,
    config: Arc<RwLock<SleepyConfig>>,
}


//TODO maintenance longest chain
//fetch miss parent
impl Chain {
    pub fn init(config: Arc<RwLock<SleepyConfig>>) -> Arc<Self> {
        let (sender, receiver) = channel();
        let mut block_headers = HashMap::new();
        let mut block_bodies = HashMap::new();
        let mut block_hashes = BTreeMap::new();
        let genesis = Block::genesis(config.read().start_time());
        let hash = genesis.hash();
        block_headers.insert(hash, genesis.header);
        block_bodies.insert(hash, genesis.body);
        block_hashes.insert(0, hash);

        let chain = Arc::new(Chain {
                                block_headers: RwLock::new(block_headers),
                                block_bodies: RwLock::new(block_bodies),
                                future_blocks: RwLock::new(Vec::new()),
                                unknown_parent: RwLock::new(HashMap::new()),
                                transaction_addresses: RwLock::new(HashMap::new()),
                                block_hashes: RwLock::new(block_hashes),
                                current_height: RwLock::new(0),
                                current_hash: RwLock::new(hash),
                                sender: Mutex::new(sender),
                                config: config,
                             });

        let mario = chain.clone();
        thread::spawn(move || loop {
                          let (height, hash) = receiver.recv().unwrap();
                          mario.adjust_block_hashes(height, hash);
                      });

        let subtask = chain.clone();
        thread::spawn(move || {
            info!("hanle pending!");
            let dur = { 1000 / subtask.config.read().hz };
            let dur = Duration::from_millis(dur);
            loop {
                thread::sleep(dur);
                subtask.handle_pending();
            }
        });
        chain
    }

    fn insert_at(&self, block: Block) {
        let hash = block.hash();
        let height = block.height;
        { self.block_headers.write().insert(hash, block.header); }
        { self.block_bodies.write().insert(hash, block.body); }

        let mut rng = thread_rng();

        let mut current_height = self.current_height.write();
        let mut current_hash = self.current_hash.write();

        if height == *current_height + 1 
           || (height == *current_height && rng.gen_range(0, 1) == 0) {
           
            self.sender.lock().send((height, hash)).unwrap();
            
            *current_height = height;
            *current_hash = hash;

        }

    }

    pub fn insert(&self, block: Block) -> Result<(), Error> {
        let hash = block.hash();

        self.block_basic_check(&block)?;
        
        self.check_transactions(&block)?;
        
        self.insert_at(block);
        
        if let Some(blocks) = self.unknown_parent.write().remove(&hash) {
            for b in blocks {
                self.insert(b)?;
            }
        }

        Ok(())
    }

    pub fn anc_height(&self, height: u64) -> u64 {
        let len = {self.config.read().epoch_len};
        let mut a = height / len;
        if a > 0 { a -= 1}
        a * len
    }

    pub fn anc_hash(&self, height: u64, hash: H256) -> Option<H256> {
        let h = self.anc_height(height + 1);
        self.block_hash_by_number_fork(h, height, hash)
    }

    pub fn tx_basic_check(&self, stx: &SignedTransaction) -> Result<(), Error> {
        stx.recover_public()?;
        Ok(())
    }

    pub fn block_basic_check(&self, block: &Block) -> Result<(), Error> {
        let hash = block.hash();

        if block.difficulty() > self.config.read().get_difficulty() {
            return Err(Error::InvalidProof);
        }

        let config = self.config.read();

        let max = config.timestamp_now() + 2 * config.hz * config.duration;

        if max < block.timestamp {
            return Err(Error::InvalidTimestamp);
        }

        let headers = self.block_headers.read();

        if headers.contains_key(&hash) {
            return Err(Error::DuplicateBlock);
        }

        match headers.get(&block.parent_hash) {
            Some(h) => {
                if block.timestamp <= h.timestamp {
                    return Err(Error::InvalidTimestamp);
                }          
            }
            None => {
                let mut unknown_parent = self.unknown_parent.write();
                let blocks = unknown_parent.entry(block.parent_hash).or_insert_with(|| Vec::new());
                blocks.push(block.clone());
                return Err(Error::UnknownParent);
            }
        }

        if block.timestamp > { config.timestamp_now() } {
            self.future_blocks.write().push(block.clone());
            return Err(Error::FutureBlock);
        }
        
        let height = block.height;
        let anc_hash = self.anc_hash(height - 1, block.parent_hash).ok_or(Error::UnknownAncestor)?;
        let sign_pub = block.sign_public()?;
        let (proof_pub, proof_g) = config.get_proof_pub(&sign_pub).ok_or(Error::InvalidPublicKey)?;

        if !block.verify_proof(anc_hash, proof_pub, proof_g) {
            return Err(Error::InvalidProofKey);
        }

        Ok(())
    }

    pub fn transactions_diff(&self, mut height: u64, mut hash: H256) -> (u64, HashSet<H256>) {
        let mut txs_set = HashSet::new();
        let block_hashes = self.block_hashes.read();

        loop {
            if Some(&hash) == block_hashes.get(&height) || height == 0 {
                break;
            }
            
            let transactions = self.block_bodies.read().get(&hash).unwrap().transactions.clone();
            for tx in transactions {
                txs_set.insert(tx.hash());
            }

            hash = self.block_headers.read().get(&hash).unwrap().parent_hash;
            height -= 1;
        }

        (height, txs_set)

    }

    pub fn check_transactions(&self, block: &Block) -> Result<(), Error> {

        let (height, mut txs_set) = self.transactions_diff(block.height - 1, block.parent_hash);

        for tx in block.body.transactions.clone() {
            let tx_hash = tx.hash();
            if txs_set.contains(&tx_hash) {
                return Err(Error::DuplicateTransaction);
            }

            if let Some(addr) = { self.transaction_addresses.read().get(&tx_hash) } {
                let block_height = {self.block_headers.read().get(&addr.block_hash).unwrap().height};
                if block_height <= height {
                    return Err(Error::DuplicateTransaction);
                }
            }

            txs_set.insert(tx_hash);
        }
        Ok(())
    }

    pub fn filter_transactions(&self, height: u64, hash: H256, txs: Vec<SignedTransaction>) -> Vec<SignedTransaction> {
        let (height, mut txs_set) = self.transactions_diff(height, hash);

        txs.into_iter().filter(|tx| {
            let tx_hash = tx.hash();
            if txs_set.contains(&tx_hash) {
                return false;
            }

            if let Some(addr) = { self.transaction_addresses.read().get(&tx_hash) } {
                let block_height = {self.block_headers.read().get(&addr.block_hash).unwrap().height};
                if block_height <= height {
                    return false;
                }
            }

            txs_set.insert(tx_hash);

            true

        }).collect()

    }

    pub fn gen_block(&self, height: u64, hash: H256, time: u64, time_sig: Vec<u8>, txs: Vec<SignedTransaction>) -> Block {
        
        let txs = self.filter_transactions(height, hash, txs);

        let signer_private_key = {self.config.read().get_signer_private_key()};

        let mut block = Block::init(height + 1, time, hash, txs, time_sig);
        
        block.sign(&signer_private_key);

        self.insert_at(block.clone());

        block
    }

    pub fn get_status(&self) -> (u64, H256) {
        let current_height = self.current_height.read();
        let current_hash = self.current_hash.read();
        (*current_height, *current_hash)
    }

    pub fn block_hash_by_number_fork(&self, number: u64, mut height: u64, mut hash: H256) -> Option<H256> {
        let block_hashes = self.block_hashes.read();

        if height < number {
            return None;
        }
        loop {
            if height == number {
                return Some(hash);
            }

            if Some(&hash) == block_hashes.get(&height) {
                return block_hashes.get(&number).map(|h| h.clone());
            }

            if let Some(header) = self.get_block_header_by_hash(&hash) {
                hash = header.parent_hash;
                height -= 1;
            } else {
                return None;
            }
        }
    }

    pub fn current_height(&self) -> u64 {
        *self.current_height.read()
    }

    pub fn block_hash_by_number(&self, number: u64) -> Option<H256> {
        self.block_hashes.read().get(&number).map(|h| h.clone())
    }

    pub fn get_block_header_by_hash(&self, hash: &H256) -> Option<Header> {
        self.block_headers.read().get(hash).cloned()
    }

    pub fn get_block_body_by_hash(&self, hash: &H256) -> Option<Body> {
        self.block_bodies.read().get(hash).cloned()
    }

    pub fn get_block_by_hash(&self, hash: &H256) -> Option<Block> {
        if let (Some(h), Some(b)) = (self.get_block_header_by_hash(hash), self.get_block_body_by_hash(hash)) {
            Some( Block {
                header: h,
                body: b,
            })
        } else {
            None
        }
    }


    pub fn adjust_block_hashes(&self, mut height: u64, mut hash: H256) {

        let mut block_hashes = self.block_hashes.write();
  
        info!("begin adjust best blocks {:?} {:?}", height, hash);

        loop {

            if let Some(h) = block_hashes.insert(height, hash) {
                if h == hash || height == 0 {
                    break;
                }
                let mut transaction_addresses = self.transaction_addresses.write();
                let transactions = self.block_bodies.read().get(&h).expect("invalid block").transactions.clone();
                for tx in transactions {
                    transaction_addresses.remove(&tx.hash());
                }

            }
                
            {
                let mut transaction_addresses = self.transaction_addresses.write();
                let transactions = self.block_bodies.read().get(&hash).expect("invalid block").transactions.clone();
                for (i, tx) in transactions.iter().enumerate() {
                    let addr = TransactionAddress{ index: i, block_hash: hash};
                    transaction_addresses.insert(tx.hash(), addr);
                }
            }

            hash = { self.block_headers.read().get(&hash).expect("invalid block").parent_hash};
            height -= 1;
        }

        info!("Chain {{");
        for (key, value) in block_hashes.iter().rev().take(10) {
            info!("   {} => {}", key, value);
        }
        info!("}}");
    }

    fn handle_pending(&self) {
        let now = self.config.read().timestamp_now();

        let left: Vec<Block> = self.future_blocks.read().clone().into_iter().filter(|b| {
            if b.timestamp <= now {
                self.insert(b.clone()).expect("insert block failed");
                false
            } else {
                true
            }
        }).collect();

        *self.future_blocks.write() = left;
    }

}