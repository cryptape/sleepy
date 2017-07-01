extern crate sha3 as sha3_ext;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rustc_serialize;
extern crate serde_json;
extern crate tiny_keccak;
extern crate bigint;

mod sha3;
pub mod error;

pub use sha3::*;

trait AsMillis {
    fn as_millis(&self) -> u64;
}

impl AsMillis for std::time::Duration {
    fn as_millis(&self) -> u64 {
        self.as_secs() * 1_000 + (self.subsec_nanos() / 1_000_000) as u64
    }
}

pub fn timestamp_now() -> u64 {
    use AsMillis;
    ::std::time::UNIX_EPOCH.elapsed().unwrap().as_millis()
}
