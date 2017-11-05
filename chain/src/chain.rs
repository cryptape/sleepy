use parking_lot::{Mutex, RwLock};
use util::hash::H256;
use std::collections::{HashMap, HashSet, VecDeque};
use rand::{thread_rng, Rng};
use util::config::SleepyConfig;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::sync::Arc;
use std::time::Duration;
use block::{Block, Body, RichHeader, BlockNumber};
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
pub struct BlockInfo {
    hash: H256,
    height: u64,
    timestamp: u64,
    transactions: Vec<H256>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Status {
    height: u64,
    hash: H256,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HashCache {
    queue: VecDeque<BlockInfo>,
    hashes: HashMap<H256, u64>,
}

impl HashCache {
    pub fn new(n: usize) -> HashCache {
        HashCache {
            queue: VecDeque::with_capacity(n),
            hashes: HashMap::new(),
        }
    }

    pub fn get(&self, i: usize) -> Option<BlockInfo> {
        self.queue.get(i).map(|b| b.clone())
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn pop_front(&mut self) {
        match self.queue.pop_front() {
            Some(b) => {
                for h in b.transactions {
                    self.hashes.remove(&h);
                }
            }
            _ => {}
        }
    }

    pub fn push_front(&mut self, b: BlockInfo) {
        let height = b.height;
        let txs = b.transactions.clone();
        for h in txs {
            self.hashes.insert(h, height);
        }
        self.queue.push_front(b);
    }

    pub fn push_back(&mut self, b: BlockInfo) {
        let height = b.height;
        let txs = b.transactions.clone();
        for h in txs {
            self.hashes.insert(h, height);
        }
        self.queue.push_back(b);
    }

    pub fn drop_back(&mut self, m: usize) {
        let bs: Vec<BlockInfo> = self.queue.drain(m..).collect();
        for b in bs {
            let txs = b.transactions;
            for h in txs {
                self.hashes.remove(&h);
            }
        }
    }

    pub fn replace(&mut self, i: usize, b: BlockInfo) {
        let txs = b.transactions.clone();
        let height = b.height;
        self.queue.push_back(b);
        match self.queue.swap_remove_back(i) {
            Some(b) => {
                for h in b.transactions {
                    self.hashes.remove(&h);
                }
            }
            _ => {}
        }

        for h in txs {
            self.hashes.insert(h, height);
        }
    }

    pub fn contains(&self, hash: &H256, left: u64, right: u64) -> bool {
        match self.hashes.get(&hash) {
            Some(height) => {
                *height <= right && *height >= left
            },
            _ => false
        }
    }
}

pub struct Chain {
    db: Arc<KeyValueDB>,
    cache_man: Mutex<CacheManager<CacheId>>,
    
    //block cache
    block_headers: RwLock<HashMap<H256, RichHeader>>,
    block_bodies: RwLock<HashMap<H256, Body>>,

    //extra caches
    transaction_addresses: RwLock<HashMap<H256, TransactionAddress>>,
    block_hashes: RwLock<HashMap<BlockNumber,H256>>,

    future_blocks: RwLock<Vec<Block>>,
    unknown_parent: RwLock<HashMap<H256, Vec<Block>>>,
    current_height: RwLock<u64>,
    current_hash: RwLock<H256>,

    txs_cache: RwLock<HashCache>,

    config: Arc<RwLock<SleepyConfig>>,
    sender: Mutex<Sender<H256>>,
}

//TODO use more efficient  way to check duplicated transactions.

impl Chain {
    pub fn init(config: Arc<RwLock<SleepyConfig>>, db: Arc<KeyValueDB>) -> Arc<Self> {
        let (sender, receiver) = channel();
        // 400 is the avarage size of the key
        let cache_man = CacheManager::new(1 << 14, 1 << 20, 400);
        let lmt = 100u64;
        let bs = {config.read().buffer_size};
       
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

                                txs_cache: RwLock::new(HashCache::new((lmt+bs+5) as usize)),

                                config: config,
                                sender: Mutex::new(sender),
                             });

        let ret = chain.db.get(db::COL_EXTRA, b"current_hash").unwrap();
        
        match ret {
            Some(hash) => {
                let mut txs_cache = chain.txs_cache.write();
                let hash = H256::from_slice(&hash);
                info!("{}", hash);
                let mut header = chain.get_block_header_by_hash(&hash).expect("header not found!");
                let mut current_height = chain.current_height.write();
                let mut current_hash = chain.current_hash.write();
                
                *current_height = header.height;
                *current_hash = hash;

                let mut n = lmt+bs+1;
                loop {
                    let txs_hashes = chain.block_transaction_hashes_by_hash(&header.hash());
                    txs_cache.push_front(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: txs_hashes.clone()});

                    n -= 1;
                    if n == 0 || header.height == 0{
                        break;
                    }

                    header = chain.get_block_header_by_hash(&header.parent_hash).expect("header not found!");
                }

                for _ in 0..n {
                    txs_cache.push_back(BlockInfo{hash: header.hash(), height: 0, timestamp: header.timestamp, transactions: Vec::new()});
                }
            }
            None => {
                let t = chain.config.read().start_time();
                let genesis = Block::genesis(t);
                {
                    let mut txs_cache = chain.txs_cache.write();
                    for _ in 0..(lmt+bs+1) {
                        txs_cache.push_back(BlockInfo{hash: genesis.hash(), height: 0, timestamp: t, transactions: Vec::new()});
                    }
                }
                chain.insert_at(genesis, true);
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

    fn insert_at(&self, block: Block, verified: bool) {
        let hash = block.hash();
        let height = block.height;

        let mut batch = self.db.transaction();
        let rh = RichHeader {header: block.header, verified: verified};

        { 
            let mut write_headers = self.block_headers.write();
            batch.write_with_cache(db::COL_HEADERS, &mut *write_headers, hash, rh.clone(), CacheUpdatePolicy::Overwrite);
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
           
            if self.adjust_block_hashes(&mut batch, rh) {
                self.save_status(&mut batch, height, hash);
            } else {
                info!("Switch Long Fork Error {:?} {:?}", height, hash);
            }
        }

        self.db.write(batch).expect("DB write failed.");

    }

    pub fn insert(&self, block: Block) -> Result<(), Error> {
        let hash = block.hash();

        self.block_basic_check(&block)?;
        
        let checked = self.check_transactions(&block)?;
        
        self.insert_at(block, checked);

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

    pub fn transactions_diff(&self, mut height: u64, mut hash: H256) -> Result<(u64, HashSet<H256>), Error> {
        let mut txs_set = HashSet::new();
        let mut bs = {self.config.read().buffer_size};
        let ch = {*self.current_height.read()};

        if height + bs < ch {
            return Err(Error::LongFork);
        }

        loop {
            if bs == 0 {
                return Err(Error::LongFork);
            }
            bs -= 1;

            if Some(hash) == self.block_hash_by_number(height) || height == 0 {
                break;
            }
            
            let txs_hashes = self.block_transaction_hashes_by_hash(&hash);
            for h in txs_hashes {
                txs_set.insert(h);
            }
            
            let header = self.get_block_header_by_hash(&hash).unwrap();
            
            if !header.verified {
                return Err(Error::LongFork);
            }

            hash = header.parent_hash;
            height -= 1;
        }

        Ok((height, txs_set))

    }

    pub fn get_left_bound(&self, height: u64, txs_cache: &HashCache) -> (u64, u64) {
        let ch = {*self.current_height.read()};
        let nps = {self.config.read().nps};
        let bs = {self.config.read().buffer_size};
        let i = bs + height - ch;

        let bi = txs_cache.get(i as usize).unwrap();
        
        (bi.height, bi.timestamp * 1000 / nps)

    }

    pub fn check_transactions(&self, block: &Block) -> Result<bool, Error> {
        let (height, mut txs_set) = match self.transactions_diff(block.height - 1, block.parent_hash) {
            Ok((h, t)) => (h, t),
            Err(_) => return Ok(false),

        };
        let txs_cache = self.txs_cache.read();
        let (bh, bt) = self.get_left_bound(block.height - 1, &txs_cache);

        for tx in block.body.transactions.clone() {
            if tx.timestamp <= bt {
                return Err(Error::OverdueTransaction);
            }
            let tx_hash = tx.hash();
            if txs_set.contains(&tx_hash) {
                return Err(Error::DuplicateTransaction);
            }

            if txs_cache.contains(&tx_hash, bh, height) {
                return Err(Error::DuplicateTransaction);
            }

            txs_set.insert(tx_hash);
        }
        Ok(true)
    }

    pub fn filter_transactions(&self, height: u64, hash: H256, txs: Vec<SignedTransaction>) -> Vec<SignedTransaction> {
        let (height, mut txs_set) = self.transactions_diff(height, hash).unwrap();
        let txs_cache = self.txs_cache.read();
        let (bh, bt) = self.get_left_bound(height, &txs_cache);

        txs.into_iter().filter(|tx| {
            let tx_hash = tx.hash();
            if tx.timestamp <= bt {
                return false;
            }
            if txs_set.contains(&tx_hash) {
                return false;
            }

            if txs_cache.contains(&tx_hash, bh, height) {
                return false;
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

        self.insert_at(block.clone(), true);

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

    pub fn get_block_header_by_hash(&self, hash: &H256) -> Option<RichHeader> {
        let result = self.db.read_with_cache(db::COL_HEADERS, &self.block_headers, hash);
		self.cache_man.lock().note_used(CacheId::BlockHeader(hash.clone()));
		result
    }

    pub fn get_block_body_by_hash(&self, hash: &H256) -> Option<Body> {
        let result = self.db.read_with_cache(db::COL_BODIES, &self.block_bodies, hash);
		self.cache_man.lock().note_used(CacheId::BlockBody(hash.clone()));
		result
    }

    pub fn get_block_body_by_height(&self, height: u64) -> Option<Body> {
        self.block_hash_by_number(height).map_or(None, |h| self.get_block_body_by_hash(&h))
    }

    pub fn get_block_by_hash(&self, hash: &H256) -> Option<Block> {
        if let (Some(h), Some(b)) = (self.get_block_header_by_hash(hash), self.get_block_body_by_hash(hash)) {
            Some( Block {
                header: h.header,
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
    
    pub fn switch_long_fork(&self, batch: &mut DBTransaction, mut header: RichHeader) -> Result<(), Error> {
        let mut headers = Vec::new();
        let mut txs_cache = {self.txs_cache.read().clone()};
        let current_height = {*self.current_height.read()};
        let best = header.height;
        
        let mut n = 0usize;

        if current_height == header.height {
            n += 1;
        }
        
        loop {
            let hash = header.parent_hash;
            headers.push(header);
            header = self.get_block_header_by_hash(&hash).unwrap();
            if header.verified {
                break;
            }
            n += 1;
        }

        let si = txs_cache.len();

        let mut fork_headers = Vec::new();

        if n < si {
            let m = si - n;
            txs_cache.drop_back(m);

            let mut height = header.height;
            let mut h = header.hash();

            for i in 0..m {
                let v = self.block_hash_by_number(height);
                if Some(h) == v {
                    break;
                }

                fork_headers.push(header.clone());

                let tx_hashes = self.block_transaction_hashes_by_hash(&h);

                txs_cache.replace(m-i-1, BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes.clone()});
                                                               
                h = header.parent_hash;
                height -= 1;
                header = self.get_block_header_by_hash(&h).unwrap();

            }

            h = txs_cache.queue.front().unwrap().hash;
            header = self.get_block_header_by_hash(&h).unwrap();

            if header.height !=0 {
                header = self.get_block_header_by_hash(&header.parent_hash).unwrap();
            }
        } else {
            n = si;
        }

        for _ in 0..n {
            let tx_hashes = self.block_transaction_hashes_by_hash(&header.hash());
            txs_cache.push_front(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes});
            if header.height != 0 {
                header = self.get_block_header_by_hash(&header.parent_hash).unwrap();
            }
        }

        let mut old_txs = Vec::new();
        let mut new_txs = Vec::new();
        let mut new_blocks = Vec::new();

        headers.reverse();
        for mut header in headers {
            new_blocks.push((header.height, header.hash()));

            let mut txs_set = HashSet::new();
            let (bh, bt) = self.get_left_bound(header.height - 1, &txs_cache);
            let txs = self.get_block_body_by_hash(&header.hash()).expect("invalid block").transactions;
            let tx_hashes: Vec<H256> = txs.iter().map(|t| t.hash()).collect();

            for tx in txs.clone() {
                let tx_hash = tx.hash();
                if tx.timestamp <= bt {
                    return Err(Error::OverdueTransaction);
                }
                if txs_set.contains(&tx_hash) {
                    return Err(Error::DuplicateTransaction);
                }

                if txs_cache.contains(&tx_hash, bh, current_height) {
                    return Err(Error::DuplicateTransaction);
                }

                txs_set.insert(tx_hash);
            }

            for (i, h) in tx_hashes.iter().enumerate() {
                let addr = TransactionAddress{ index: i, block_hash: header.hash()};
                new_txs.push((*h, addr)); 
            }

            let old = self.block_transaction_hashes_by_height(header.height);
            old_txs.extend_from_slice(&old);
            
            {   
                header.verified = true;
                let mut write_headers = self.block_headers.write();
                batch.write_with_cache(db::COL_HEADERS, &mut *write_headers, header.hash(), header.clone(), CacheUpdatePolicy::Overwrite);
            }

            txs_cache.pop_front();
            txs_cache.push_back(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes});

        }
        {
            let mut txs_cache_write = self.txs_cache.write();
            *txs_cache_write = txs_cache;
        }

        //update blocknumber tx
        if !fork_headers.is_empty() {
            let mut header = fork_headers.last().unwrap().clone();
            let mut hash = header.parent_hash;
            let mut height = header.height - 1;
            loop {
                if Some(hash) == self.block_hash_by_number(height) {
                    break;
                }
                header = self.get_block_header_by_hash(&hash).unwrap();
                hash = header.parent_hash;
                fork_headers.push(header);
                height -= 1;
            }
        }

        fork_headers.reverse();

        let mut old_txs_left = Vec::new();
        let mut new_txs_left = Vec::new();

        for header in fork_headers {
            new_blocks.push((header.height, header.hash()));

            let hash = header.hash();
            let height = header.height;

            let old = self.block_transaction_hashes_by_height(height);
            old_txs.extend_from_slice(&old);

            let tx_hashes = self.block_transaction_hashes_by_hash(&hash);
            
            for (i, h) in tx_hashes.iter().enumerate() {
                let addr = TransactionAddress{ index: i, block_hash: hash};
                new_txs_left.push((*h, addr)); 
            }

        }

        old_txs_left.extend_from_slice(&old_txs);
        new_txs_left.extend_from_slice(&new_txs);

        self.update_transaction_addresses(batch, old_txs_left, new_txs_left);
        self.update_block_number(batch, new_blocks);

        self.print_chain(best);

        Ok(())
    }

    pub fn block_transaction_hashes_by_height(&self, height: u64) -> Vec<H256> {
        self.get_block_body_by_height(height).expect("invalid block")
                                             .transactions
                                             .iter().map(|t| t.hash()).collect()

    }

    pub fn block_transaction_hashes_by_hash(&self, hash: &H256) -> Vec<H256> {
        self.get_block_body_by_hash(hash).expect("invalid block")
                                         .transactions
                                         .iter().map(|t| t.hash()).collect()
    }

    pub fn update_block_number(&self, batch: &mut DBTransaction, hashes: Vec<(u64, H256)>) {
        let mut block_hashes = self.block_hashes.write();
        for (height, hash) in hashes {
            batch.write_with_cache(db::COL_EXTRA, &mut *block_hashes, height, hash, CacheUpdatePolicy::Overwrite);
            self.cache_man.lock().note_used(CacheId::BlockHashes(height));
        }
    }

    pub fn update_transaction_addresses(&self, batch: &mut DBTransaction, old: Vec<H256>, new: Vec<(H256, TransactionAddress)>) {
        let mut transaction_addresses = self.transaction_addresses.write();

        for h in old {
            batch.delete_with_cache(db::COL_EXTRA, &mut *transaction_addresses, h);
        }

        for (h, addr) in new {
            batch.write_with_cache(db::COL_EXTRA, &mut *transaction_addresses, h, addr, CacheUpdatePolicy::Overwrite);
            self.cache_man.lock().note_used(CacheId::TransactionAddresses(h)); 
        }

    }

    //TODO: get old hash from queue
    pub fn adjust_block_hashes(&self, batch: &mut DBTransaction, mut header: RichHeader) -> bool {
        info!("begin adjust best blocks {:?} {:?}", header.height, header.hash());
        if !header.verified {
            return self.switch_long_fork(batch, header).is_ok();
        }
        let best = header.height;
        let mut height = header.height;
        
        let mut txs_cache = self.txs_cache.write();
        let mut tx_hashes: Vec<H256>;

        let mut i = txs_cache.len() - 1;

        let mut old_txs = Vec::new();
        let mut new_txs = Vec::new();
        let mut block_hashes = Vec::new();

        loop {
            let old = self.block_hash_by_number(height);
            let hash = header.hash();

            if let Some(h) = old {
                if h == hash {
                    break;
                }

                old_txs.extend_from_slice(&self.block_transaction_hashes_by_hash(&h));
                
                tx_hashes = self.block_transaction_hashes_by_hash(&hash);

                txs_cache.replace(i, BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes.clone()});

                i -= 1;

            } else {
                txs_cache.pop_front();

                tx_hashes = self.block_transaction_hashes_by_hash(&hash);

                txs_cache.push_back(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes.clone()});
            }
            
            block_hashes.push((height, hash)); 

            for (i, h) in tx_hashes.iter().enumerate() {
                let addr = TransactionAddress{ index: i, block_hash: hash};
                new_txs.push((*h, addr)); 
            }

            if height == 0 {
                break;
            }
            
            header = self.get_block_header_by_hash(&header.parent_hash).expect("invalid block");
            height -= 1;
        }

        self.update_transaction_addresses(batch, old_txs, new_txs);
        self.update_block_number(batch, block_hashes);

        self.print_chain(best);
        true
    }

    fn print_chain(&self, best: u64) {
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