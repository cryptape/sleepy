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

pub mod error;
pub mod config;

mod sha3;

pub use sha3::*;

pub fn timestamp_now() -> u64 {
    let now = time::now().to_timespec();
    (now.sec * 10 + now.nsec as i64 / 100000000) as u64
}
