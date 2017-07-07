extern crate sha3 as sha3_ext;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rustc_serialize;
extern crate serde_json;
extern crate rand;
extern crate tiny_keccak;
extern crate bigint;
extern crate time;
extern crate parking_lot;
extern crate rlp;
extern crate elastic_array;
extern crate heapsize;
extern crate itertools;

#[macro_use]
extern crate log as rlog;

pub mod error;
pub mod config;
pub mod hashdb;
pub mod common;
pub mod memorydb;
pub mod bytes;
pub mod standard;
pub mod sha3;
pub mod avl;
pub mod pki;

pub use common::*;
pub use hashdb::*;
pub use memorydb::MemoryDB;

pub fn timestamp_now() -> u64 {
    let now = time::now().to_timespec();
    (now.sec * 10 + now.nsec as i64 / 100000000) as u64
}
