extern crate util;
extern crate crypto;
extern crate bigint;
extern crate chain;
#[macro_use]
extern crate log;
extern crate network;
extern crate bincode;
extern crate parking_lot;
extern crate timesync;

use std::sync::mpsc::Sender;
use chain::Chain;
use std::thread;
use std::time::Duration;
use crypto::{sign, KeyPair};
use bigint::hash::{H256, H520};
use util::Hashable;
use std::sync::Arc;
use network::connection::Operation;
use util::config::SleepyConfig;
use bincode::{serialize, Infinite};
use parking_lot::RwLock;
use network::msgclass::MsgClass;
use timesync::{TimeSyncer}; 

pub fn start_miner(tx: Sender<(u32, Operation, Vec<u8>)>,
                   chain: Arc<Chain>,
                   config: Arc<RwLock<SleepyConfig>>,
                   time_syncer: Arc<RwLock<TimeSyncer>>) {
    let difficulty: H256 = {
        config.read().get_difficulty().into()
    };
    let tx = tx.clone();
    let chain = chain.clone();
    let config = config.clone();
    let time_syncer = time_syncer.clone();
    thread::spawn(move || {
        info!("start mining!");
        loop {
            let t: u64 = {time_syncer.read().time_now_ms()} * {config.read().hz} / 1000;
            let miner_privkey = {
                config.read().get_miner_private_key()
            };
            let miner_keypair = KeyPair::from_privkey(miner_privkey).unwrap();
            let sig: H520 = sign(miner_keypair.privkey(), &H256::from(t))
                .unwrap()
                .into();
            let hash = sig.sha3();
            if hash < difficulty {               
                let id = {config.read().getid()};
                let signed_blk = chain.gen_block(t, sig, *miner_keypair.pubkey());
                info!("generate block at timestamp {}", t);
                let msg = MsgClass::BLOCK(signed_blk);
                let message = serialize(&msg, Infinite).unwrap();
                tx.send((id, Operation::BROADCAST, message)).unwrap();             
            }
            thread::sleep(Duration::from_millis(1000 / {config.read().hz}));
        }
    });
}
