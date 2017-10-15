use parking_lot::{Mutex, RwLock};
use util::hash::H256;
use std::collections::{HashMap, HashSet};
use rand::{thread_rng, Rng};
use util::config::SleepyConfig;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::sync::Arc;
use std::time::Duration;
use block::{Block, Body, Header, BlockNumber};
use transaction::SignedTransaction;
use error::*;
use kvdb::{DBTransaction, KeyValueDB};
use cache_manager::CacheManager;
use extras::*;
use db::{self, Writable, Readable, CacheUpdatePolicy};
use cache::*;
use heapsize::HeapSizeOf;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
enum CacheId {
	BlockHeader(H256),
	BlockBody(H256),
	BlockHashes(BlockNumber),
	TransactionAddresses(H256),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Status {
    height: u64,
    hash: H256,
}

pub struct Chain {
    db: Arc<KeyValueDB>,
    cache_man: Mutex<CacheManager<CacheId>>,
    
    //block cache
    block_headers: RwLock<HashMap<H256, Header>>,
    block_bodies: RwLock<HashMap<H256, Body>>,

    //extra caches
    transaction_addresses: RwLock<HashMap<H256, TransactionAddress>>,
    block_hashes: RwLock<HashMap<BlockNumber,H256>>,

    future_blocks: RwLock<Vec<Block>>,
    unknown_parent: RwLock<HashMap<H256, Vec<Block>>>,
    current_height: RwLock<u64>,
    current_hash: RwLock<H256>,
    config: Arc<RwLock<SleepyConfig>>,
    sender: Mutex<Sender<H256>>,
}

//TODO use more efficient  way to check duplicated transactions.

impl Chain {
    pub fn init(config: Arc<RwLock<SleepyConfig>>, db: Arc<KeyValueDB>) -> Arc<Self> {
        let (sender, receiver) = channel();
        // 400 is the avarage size of the key
        let cache_man = CacheManager::new(1 << 14, 1 << 20, 400);
       
        let chain = Arc::new(Chain {
                                db: db.clone(),
                                cache_man: Mutex::new(cache_man),
                                block_headers: RwLock::new(HashMap::new()),
                                block_bodies: RwLock::new(HashMap::new()),
                                future_blocks: RwLock::new(Vec::new()),
                                unknown_parent: RwLock::new(HashMap::new()),
                                transaction_addresses: RwLock::new(HashMap::new()),
                                block_hashes: RwLock::new(HashMap::new()),
                                current_height: RwLock::new(0),
                                current_hash: RwLock::new(H256::default()),
                                config: config,
                                sender: Mutex::new(sender),
                             });

        let ret = chain.db.get(db::COL_EXTRA, b"current_hash").unwrap();
        
        match ret {
            Some(hash) => {
                let hash = H256::from_slice(&hash);
                info!("{}", hash);
                let header = chain.get_block_header_by_hash(&hash).expect("header not found!");
                let mut current_height = chain.current_height.write();
                let mut current_hash = chain.current_hash.write();
                
                *current_height = header.height;
                *current_hash = hash;
            }
            None => {
                let genesis = Block::genesis(chain.config.read().start_time());
                chain.insert_at(genesis);

            }

        }

        let mario = chain.clone();
        thread::spawn(move || loop {
                            let hash = receiver.recv().unwrap();
                            if let Some(blocks) = mario.unknown_parent.write().remove(&hash) {
                                for b in blocks {
                                   let _ = mario.insert(b);                            
                                }
                            }
                      });

        let subtask = chain.clone();
        thread::spawn(move || {
            info!("hanle pending!");
            let dur = { 1000 / subtask.config.read().nps };
            let dur = Duration::from_millis(dur);
            loop {
                thread::sleep(dur);
                subtask.handle_pending();
            }
        });
        chain
    }

    fn save_status(&self, batch: &mut DBTransaction, height: BlockNumber, hash: H256) {
        batch.put(db::COL_EXTRA, b"current_hash", &hash);
        
        let mut current_height = self.current_height.write();
        let mut current_hash = self.current_hash.write();
        
        *current_height = height;
        *current_hash = hash;

    }

    fn insert_at(&self, block: Block) {
        let hash = block.hash();
        let height = block.height;

        let mut batch = self.db.transaction();

        { 
            let mut write_headers = self.block_headers.write();
            batch.write_with_cache(db::COL_HEADERS, &mut *write_headers, hash, block.header, CacheUpdatePolicy::Overwrite);
        }
        {
            let mut write_bodies = self.block_bodies.write();
            batch.write_with_cache(db::COL_BODIES, &mut *write_bodies, hash, block.body, CacheUpdatePolicy::Overwrite);
        }

        let mut rng = thread_rng();

        let current_height = { *self.current_height.read() };
        let current_hash = { *self.current_hash.read() };

        if height == current_height + 1 
           || (height == current_height && (rng.gen_range(0, 1) == 0 || current_hash == H256::default())) {
           
            self.adjust_block_hashes(&mut batch, height, hash);
            self.save_status(&mut batch, height, hash);

        }

        self.db.write(batch).expect("DB write failed.");

    }

    pub fn insert(&self, block: Block) -> Result<(), Error> {
        let hash = block.hash();

        self.block_basic_check(&block)?;
        
        self.check_transactions(&block)?;
        
        self.insert_at(block);

        self.sender.lock().send(hash).unwrap();

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

        let now = match config.ntp_now() {
            Some(t) => t,
            _ => return Err(Error::NTPError),
        };

        let max = now + 2 * config.nps * config.steps;

        if max < block.timestamp {
            return Err(Error::InvalidTimestamp);
        }

        if self.get_block_header_by_hash(&hash) != None {
            return Err(Error::DuplicateBlock);
        }

        match self.get_block_header_by_hash(&block.parent_hash) {
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

        if block.timestamp > now {
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

        loop {
            if Some(hash) == self.block_hash_by_number(height) || height == 0 {
                break;
            }
            
            let transactions = self.get_block_body_by_hash(&hash).unwrap().transactions.clone();
            for tx in transactions {
                txs_set.insert(tx.hash());
            }

            hash = self.get_block_header_by_hash(&hash).unwrap().parent_hash;
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

            if let Some(addr) = self.get_transaction_address(&tx_hash) {
                let block_height = self.get_block_header_by_hash(&addr.block_hash).unwrap().height;
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

            if let Some(addr) = self.get_transaction_address(&tx_hash) {
                let block_height = self.get_block_header_by_hash(&addr.block_hash).unwrap().height;
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
        if height < number {
            return None;
        }

        loop {
            if height == number {
                return Some(hash);
            }

            if Some(hash) == self.block_hash_by_number(height) {
                return self.block_hash_by_number(number).map(|h| h.clone());
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
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.block_hashes, &number);
		self.cache_man.lock().note_used(CacheId::BlockHashes(number));
		result
    }

    pub fn get_block_header_by_hash(&self, hash: &H256) -> Option<Header> {
        let result = self.db.read_with_cache(db::COL_HEADERS, &self.block_headers, hash);
		self.cache_man.lock().note_used(CacheId::BlockHeader(hash.clone()));
		result
    }

    pub fn get_block_body_by_hash(&self, hash: &H256) -> Option<Body> {
        let result = self.db.read_with_cache(db::COL_BODIES, &self.block_bodies, hash);
		self.cache_man.lock().note_used(CacheId::BlockBody(hash.clone()));
		result
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

    /// Get the address of transaction with given hash.
	pub fn get_transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
		let result = self.db.read_with_cache(db::COL_EXTRA, &self.transaction_addresses, hash);
		self.cache_man.lock().note_used(CacheId::TransactionAddresses(hash.clone()));
		result
	}


    pub fn adjust_block_hashes(&self, batch: &mut DBTransaction, mut height: u64, mut hash: H256) {
  
        let best = height;
        info!("begin adjust best blocks {:?} {:?}", height, hash);

        loop {
            let old = self.block_hash_by_number(height);

            if let Some(h) = old {
                if h == hash {
                    break;
                }

                let transactions = self.get_block_body_by_hash(&h).expect("invalid block").transactions.clone();
                
                let mut transaction_addresses = self.transaction_addresses.write();
                for tx in transactions {
                     batch.delete_with_cache(db::COL_EXTRA, &mut *transaction_addresses, tx.hash());
                }

            }

            {
                let mut block_hashes = self.block_hashes.write();
                batch.write_with_cache(db::COL_EXTRA, &mut *block_hashes, height, hash, CacheUpdatePolicy::Overwrite);
                self.cache_man.lock().note_used(CacheId::BlockHashes(height));
            }   

            {
                let transactions = self.get_block_body_by_hash(&hash).expect("invalid block").transactions.clone();

                let mut transaction_addresses = self.transaction_addresses.write();
                for (i, tx) in transactions.iter().enumerate() {
                    let addr = TransactionAddress{ index: i, block_hash: hash};
                    batch.write_with_cache(db::COL_EXTRA, &mut *transaction_addresses, tx.hash(), addr, CacheUpdatePolicy::Overwrite);
                    self.cache_man.lock().note_used(CacheId::TransactionAddresses(tx.hash()));
        
                }
            }

            if height == 0 {
                break;
            }

            hash = { self.get_block_header_by_hash(&hash).expect("invalid block").parent_hash};
            height -= 1;
        }

        info!("Chain {{");
        
        let limit = match best > 10 {
            true => 10,
            false => best,
        };

        for i in 0..limit {
            let hash = self.block_hash_by_number(best-i).expect("invaild block number");
            info!("   {} => {}", best -i, hash);
        }

        info!("}}");
    }

    fn handle_pending(&self) {
        if let Some(now) = self.config.read().ntp_now() {

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

    /// Get current cache size.
	pub fn cache_size(&self) -> CacheSize {
		CacheSize {
			blocks: self.block_headers.read().heap_size_of_children() + self.block_bodies.read().heap_size_of_children(),
			transaction_addresses: self.transaction_addresses.read().heap_size_of_children(),
		}
	}

	/// Ticks our cache system and throws out any old data.
	pub fn collect_garbage(&self) {
		let current_size = self.cache_size().total();

		let mut block_headers = self.block_headers.write();
		let mut block_bodies = self.block_bodies.write();
		let mut block_hashes = self.block_hashes.write();
		let mut transaction_addresses = self.transaction_addresses.write();

		let mut cache_man = self.cache_man.lock();
		cache_man.collect_garbage(current_size, | ids | {
			for id in &ids {
				match *id {
					CacheId::BlockHeader(ref h) => { block_headers.remove(h); },
					CacheId::BlockBody(ref h) => { block_bodies.remove(h); },
					CacheId::BlockHashes(ref h) => { block_hashes.remove(h); }
					CacheId::TransactionAddresses(ref h) => { transaction_addresses.remove(h); }
				}
			}

			block_headers.shrink_to_fit();
			block_bodies.shrink_to_fit();
			block_hashes.shrink_to_fit();
			transaction_addresses.shrink_to_fit();

			block_headers.heap_size_of_children() +
			block_bodies.heap_size_of_children() +
			block_hashes.heap_size_of_children() +
			transaction_addresses.heap_size_of_children()
		});
	}

}