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
    total: usize,
    queue: VecDeque<BlockInfo>,
    hashes: HashMap<H256, u64>,
}

impl HashCache {
    pub fn new(n: usize, t: usize) -> HashCache {
        HashCache {
            total: t,
            queue: VecDeque::with_capacity(n),
            hashes: HashMap::new(),
        }
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn get(&self, i: usize) -> Option<BlockInfo> {
        self.queue.get(i).map(|b| b.clone())
    }

    pub fn block_info(&self, h: u64) -> Option<BlockInfo> {
        let best = self.best_height();
        if h > best {
            return None;
        }
        let i = (best - h) as usize;
        let len = self.queue.len();
        match i >= len {
            true => None,
            false => self.queue.get(len-1-i).map(|b| b.clone()),
        }

    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn best_height(&self) -> u64 {
        self.queue.back().unwrap().height
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

                                txs_cache: RwLock::new(HashCache::new((lmt+bs+5) as usize, (lmt+bs+1) as usize)),

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
        let (height, txs_set) = match self.transactions_diff(block.height - 1, block.parent_hash) {
            Ok((h, t)) => (h, t),
            Err(_) => return Ok(false),

        };
        let txs_cache = self.txs_cache.read();

        self.transactions_check(&block.body.transactions, txs_set, &txs_cache, block.height, height)?;

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
        {
            let txs_cache = self.txs_cache.read();
            if let Some(b) = txs_cache.block_info(number) {
                return Some(b.hash);
            }
        }
        self.block_hash_by_number_db(number)
    }

    pub fn block_hash_by_number_db(&self, number: u64) -> Option<H256> {
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

    fn get_unverif_headers(&self, mut header: RichHeader) -> Vec<RichHeader> {
        let mut headers: Vec<RichHeader> = Vec::new();
        loop {
            if header.verified {
                break;
            }
            let hash = header.parent_hash;
            headers.push(header);
            header = self.get_block_header_by_hash(&hash).unwrap();
        }
        headers
    }

    // use cache to get txs
    pub fn get_fork_chain(&self, mut height: u64, mut hash: H256) -> Vec<BlockInfo> {
        let mut blocks = Vec::new();
        loop {
            let v = self.block_hash_by_number(height);
            if Some(hash) == v || height == 0 {
                break;
            }

            let header = self.get_block_header_by_hash(&hash).unwrap();
            let tx_hashes = self.block_transaction_hashes_by_hash(&hash);
            blocks.push(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes});
            hash = header.parent_hash;
            height -= 1;
        }
        blocks
    }

    pub fn update_txs_cache(&self, txs_cache: &mut HashCache, blocks: Vec<BlockInfo>, m: usize) {
        let mut i = 1usize;
        for b in blocks {
            if i > m {
                break;
            }
            txs_cache.replace(m-i, b);
            i += 1;
        }
    }

    pub fn tx_cache_roll_back(&self, txs_cache: &mut HashCache, hash: H256, n: usize) {
        txs_cache.drop_back(n);
        let mut header = self.get_block_header_by_hash(&hash).unwrap();
        for _ in 0..n {
            if header.height != 0 {
                header = self.get_block_header_by_hash(&header.parent_hash).unwrap();
            }
            let tx_hashes = self.block_transaction_hashes_by_hash(&header.hash());
            txs_cache.push_front(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes});
        }
    }

    pub fn transactions_check(&self, txs: &Vec<SignedTransaction>, mut txs_set: HashSet<H256>, txs_cache: &HashCache, height: u64, max: u64) -> Result<(), Error> {
        let (bh, bt) = self.get_left_bound(height - 1, txs_cache);
        for tx in txs {
            let tx_hash = tx.hash();
            if tx.timestamp <= bt {
                return Err(Error::OverdueTransaction);
            }
            if txs_set.contains(&tx_hash) {
                return Err(Error::DuplicateTransaction);
            }

            if txs_cache.contains(&tx_hash, bh, max) {
                return Err(Error::DuplicateTransaction);
            }

            txs_set.insert(tx_hash);
        }
        Ok(())
    }
    
    pub fn switch_long_fork(&self, batch: &mut DBTransaction, header: RichHeader) -> Result<(), Error> {
        let mut txs_cache = {self.txs_cache.read().clone()};
        let current_height = {*self.current_height.read()};
        // let best = header.height;
        
        let mut headers = self.get_unverif_headers(header.clone());

        let mut n = headers.len();

        if current_height != header.height {
            n -= 1;
        }
        
        let to = txs_cache.total();
        //get fork headers
        let mut fork_blocks = self.get_fork_chain(header.height, header.hash());
        
        //get the last header
        let hash = match n < to {
            true => {
                self.update_txs_cache(&mut txs_cache, fork_blocks.clone(), to - n);
                txs_cache.queue.front().unwrap().hash
            },
            false => {
                n = to;
                headers.last().unwrap().hash()
            }
        };

        self.tx_cache_roll_back(&mut txs_cache, hash, n);

        headers.reverse();
        for mut header in headers {

            let txs = self.get_block_body_by_hash(&header.hash()).expect("invalid block").transactions;
            let tx_hashes: Vec<H256> = txs.iter().map(|t| t.hash()).collect();
            
            //check transactions
            self.transactions_check(&txs, HashSet::new(), &txs_cache, header.height, header.height)?;

            //mark header as verified
            {   
                header.verified = true;
                let mut write_headers = self.block_headers.write();
                batch.write_with_cache(db::COL_HEADERS, &mut *write_headers, header.hash(), header.clone(), CacheUpdatePolicy::Overwrite);
            }

            let b = BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: tx_hashes};
            txs_cache.pop_front();
            txs_cache.push_back(b.clone());
            fork_blocks.push(b);

        }

        self.update_transaction_addresses(batch, fork_blocks.clone());
        self.update_block_number(batch, fork_blocks);

        //update chain's txs_cache
        {
            let mut txs_cache_write = self.txs_cache.write();
            *txs_cache_write = txs_cache;
        }

        self.print_chain(header.height);

        Ok(())
    }

    pub fn block_transaction_hashes_by_height(&self, height: u64) -> Vec<H256> {
        if height > { *self.current_height.read() } {
            return Vec::new();
        }
        {
            let txs_cache = self.txs_cache.read();
            if let Some(b) = txs_cache.block_info(height) {
                return b.transactions;
            }
        }
        self.block_transaction_hashes_by_height_db(height)
    }

    pub fn block_transaction_hashes_by_height_db(&self, height: u64) -> Vec<H256> {
        self.get_block_body_by_height(height).expect("invalid block")
                                             .transactions
                                             .iter().map(|t| t.hash()).collect()
    }

    pub fn block_transaction_hashes_by_hash(&self, hash: &H256) -> Vec<H256> {
        self.get_block_body_by_hash(hash).expect("invalid block")
                                         .transactions
                                         .iter().map(|t| t.hash()).collect()
    }

    pub fn update_block_number(&self, batch: &mut DBTransaction, blocks: Vec<BlockInfo>) {
        let mut block_hashes = self.block_hashes.write();
        for b in blocks {
            batch.write_with_cache(db::COL_EXTRA, &mut *block_hashes, b.height, b.hash, CacheUpdatePolicy::Overwrite);
            self.cache_man.lock().note_used(CacheId::BlockHashes(b.height));
        }
    }

    pub fn update_transaction_addresses(&self, batch: &mut DBTransaction, blocks: Vec<BlockInfo>) {
        let mut old_txs = Vec::new();
        let mut new_txs = Vec::new();

        for b in blocks {
            let tx_hashes = b.transactions;
            let hash = b.hash;
            let height = b.height;

            let old = self.block_transaction_hashes_by_height(height);
            old_txs.extend_from_slice(&old);
            
            for (i, h) in tx_hashes.iter().enumerate() {
                let addr = TransactionAddress{ index: i, block_hash: hash};
                new_txs.push((*h, addr)); 
            }

        }

        let mut transaction_addresses = self.transaction_addresses.write();

        for h in old_txs {
            batch.delete_with_cache(db::COL_EXTRA, &mut *transaction_addresses, h);
        }

        for (h, addr) in new_txs {
            batch.write_with_cache(db::COL_EXTRA, &mut *transaction_addresses, h, addr, CacheUpdatePolicy::Overwrite);
            self.cache_man.lock().note_used(CacheId::TransactionAddresses(h)); 
        }

    }

    //TODO: get old hash from queue
    pub fn adjust_block_hashes(&self, batch: &mut DBTransaction, header: RichHeader) -> bool {
        info!("begin adjust best blocks {:?} {:?}", header.height, header.hash());

        if !header.verified {
            return self.switch_long_fork(batch, header).is_ok();
        }

        let mut fork_blocks = match header.height > 0 {
            true => self.get_fork_chain(header.height-1, header.parent_hash),
            false => Vec::new(),
        };

        let txs_hashes = self.block_transaction_hashes_by_hash(&header.hash());
        fork_blocks.push(BlockInfo{hash: header.hash(), height: header.height, timestamp: header.timestamp, transactions: txs_hashes.clone()});
        
        self.update_transaction_addresses(batch, fork_blocks.clone());

        self.update_block_number(batch, fork_blocks.clone());

        {
            let current_height = { *self.current_height.read()}; 
            let mut txs_cache = self.txs_cache.write();
            let len = txs_cache.total();
            if current_height < header.height {
                //left shift
                let front = txs_cache.queue.pop_front().unwrap();
                txs_cache.queue.push_back(front);
            } 
            self.update_txs_cache(&mut txs_cache, fork_blocks, len);
        }
        
        self.print_chain(header.height);
        true
    }

    fn print_chain(&self, best: u64) {
        info!("Chain {{");
        
        let limit = match best > 10 {
            true => 10,
            false => best,
        } + 1;

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