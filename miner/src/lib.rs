extern crate util;
extern crate crypto;
extern crate bigint;
extern crate chain;
#[macro_use]
extern crate log;
extern crate network;
extern crate bincode;
extern crate parking_lot;

use std::sync::mpsc::Sender;
use chain::{Block, Chain};
use std::thread;
use std::time::Duration;
use crypto::{PrivKey, sign, KeyPair};
use bigint::hash::{H256, H520, H512};
use bigint::uint::U256;
use util::Hashable;
use std::sync::Arc;
use network::connection::Operation;
use util::config::SleepyConfig;
use bincode::{serialize, deserialize, Infinite};
use parking_lot::RwLock;
use network::msgclass::MsgClass;
use util::timestamp_now;

pub fn start_miner(tx: Sender<(u32, Operation, Vec<u8>)>,
                   chain: Arc<Chain>,
                   config: Arc<RwLock<SleepyConfig>>) {
    let difficulty: H256 = {
        config.read().get_difficulty().into()
    };
    let tx = tx.clone();
    let chain = chain.clone();
    let config = config.clone();
    thread::spawn(move || {
        info!("start mining!");
        loop {
            let t: u64 = timestamp_now();
            let miner_privkey = {
                config.read().get_miner_private_key()
            };
            let miner_keypair = KeyPair::from_privkey(miner_privkey).unwrap();
            let sig: H520 = sign(miner_keypair.privkey(), &H256::from(t))
                .unwrap()
                .into();
            let hash = sig.sha3();
            if hash < difficulty {
                loop {
                    let (h, pre_hash) = chain.get_status();
                    let blk = Block::new(h + 1, t, pre_hash, *miner_keypair.pubkey(), sig.into());

                    let (id, signer_privkey) = {
                        let guard = config.read();
                        (guard.getid(), guard.get_signer_private_key())
                    };
                    let keypair = KeyPair::from_privkey(signer_privkey).unwrap();
                    let signed_blk = blk.sign(&keypair);
                    let ret = chain.insert(&signed_blk);
                    if ret.is_ok() {
                        info!("gerate block height {} timestamp {}", h + 1, t);
                        let msg = MsgClass::BLOCK(signed_blk);
                        let message = serialize(&msg, Infinite).unwrap();
                        tx.send((id, Operation::BROADCAST, message)).unwrap();
                        break;
                    }
                }
            }

            thread::sleep(Duration::from_millis(100));
        }
    });
}
