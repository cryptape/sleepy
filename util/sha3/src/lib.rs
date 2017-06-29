extern {
	pub fn sha3_256(out: *mut u8, outlen: usize, input: *const u8, inputlen: usize) -> i32;
	pub fn sha3_512(out: *mut u8, outlen: usize, input: *const u8, inputlen: usize) -> i32;
}

extern crate tiny_keccak;
extern crate rand;
extern crate rustc_serialize;
extern crate bigint;
extern crate libc;
#[macro_use]
extern crate heapsize;


pub mod sha3;
pub mod hash;

