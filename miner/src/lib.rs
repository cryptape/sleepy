extern crate util;
extern crate crypto;
extern crate chain;
#[macro_use]
extern crate log;
extern crate network;
extern crate bincode;
extern crate parking_lot;
extern crate tx_pool;

use std::sync::mpsc::Sender;
use chain::chain::Chain;
use std::thread;
use std::time::Duration;
use crypto::sign;
use util::hash::{H256, H520};
use util::Hashable;
use std::sync::Arc;
use network::connection::Operation;
use util::config::SleepyConfig;
use bincode::{serialize, Infinite};
use parking_lot::RwLock;
use network::msgclass::MsgClass;
use tx_pool::Pool;

pub fn start_miner(tx: Sender<(u32, Operation, Vec<u8>)>,
                   chain: Arc<Chain>,
                   config: Arc<RwLock<SleepyConfig>>,
                   tx_pool: Arc<RwLock<Pool>>) {
    
    let tx = tx.clone();
    let chain = chain.clone();
    let config = config.clone();
    let tx_pool = tx_pool.clone();
    thread::spawn(move || {
        info!("start mining!");
        loop {
            let t: u64 = {config.read().timestamp_now()};
            let miner_privkey = {config.read().get_miner_private_key()};
            let sig: H520 = sign(&miner_privkey, &H256::from(t)).unwrap().into();
            let hash = sig.sha3();
            let difficulty: H256 = {config.read().get_difficulty().into()};

            if hash < difficulty {               
                let id = {config.read().get_id()};
                let (tx_list, hash_list) = { tx_pool.write().package() };
                let signed_blk = chain.gen_block(t, sig, tx_list);
                { tx_pool.write().update(&hash_list) };
                info!("generate block at timestamp {}", t);
                let msg = MsgClass::BLOCK(signed_blk);
                let message = serialize(&msg, Infinite).unwrap();
                tx.send((id, Operation::BROADCAST, message)).unwrap();             
            }
            thread::sleep(Duration::from_millis(1000 / {config.read().hz}));
        }
    });
}
