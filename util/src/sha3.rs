//! Wrapper around tiny-keccak crate as well as common hash constants.

use std::io;
use sha3_ext::tiny_keccak::Keccak;
use sha3_ext::hash::H256;
use sha3_ext::sha3_256;
use hash::H256 as Hash256;

/// Get the SHA3 (i.e. Keccak) hash of the empty bytes string.
pub const SHA3_EMPTY: H256 = H256([0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e,
                                   0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0, 0xe5, 0x00, 0xb6, 0x53,
                                   0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85,
                                   0xa4, 0x70]);

/// The SHA3 of the RLP encoding of empty data.
pub const SHA3_NULL_RLP: H256 = H256([0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83,
                                      0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b,
                                      0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63,
                                      0xb4, 0x21]);

/// The SHA3 of the RLP encoding of empty list.
pub const SHA3_EMPTY_LIST_RLP: H256 = H256([0x1d, 0xcc, 0x4d, 0xe8, 0xde, 0xc7, 0x5d, 0x7a, 0xab,
                                            0x85, 0xb5, 0x67, 0xb6, 0xcc, 0xd4, 0x1a, 0xd3, 0x12,
                                            0x45, 0x1b, 0x94, 0x8a, 0x74, 0x13, 0xf0, 0xa1, 0x42,
                                            0xfd, 0x40, 0xd4, 0x93, 0x47]);

pub trait Hashable {
    /// Calculate SHA3 of this object.
    fn sha3(&self) -> Hash256;

    /// Calculate SHA3 of this object and place result into dest.
    fn sha3_into(&self, dest: &mut [u8]);
}

impl<T> Hashable for T
    where T: AsRef<[u8]>
{
    fn sha3(&self) -> Hash256 {
        let mut ret: H256 = H256::zero();
        self.sha3_into(&mut *ret);
        ret.into()
    }
    fn sha3_into(&self, dest: &mut [u8]) {
        let input: &[u8] = self.as_ref();

        unsafe {
            sha3_256(dest.as_mut_ptr(), dest.len(), input.as_ptr(), input.len());
        }
    }
}

/// Calculate SHA3 of given stream.
pub fn sha3(r: &mut io::BufRead) -> Result<Hash256, io::Error> {
    let mut output = [0u8; 32];
    let mut input = [0u8; 1024];
    let mut sha3 = Keccak::new_keccak256();

    // read file
    loop {
        let some = try!(r.read(&mut input));
        if some == 0 {
            break;
        }
        sha3.update(&input[0..some]);
    }

    sha3.finalize(&mut output);
    Ok(Hash256::from(H256(output)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::env;
    use std::io::{Write, BufReader};
    use rand::random;
    use std::str::FromStr;
    use super::*;

    #[test]
    fn sha3_empty() {
        assert_eq!([0u8; 0].sha3(), Hash256(SHA3_EMPTY));
    }
    #[test]
    fn sha3_as() {
        assert_eq!([0x41u8; 32].sha3(),
                   Hash256::from_str("59cad5948673622c1d64e2322488bf01619f7ff45789741b15a9f782ce9290a8").unwrap());
    }

    pub fn random_str(len: usize) -> String {
        (0..len)
            .map(|_| ((random::<f32>() * 26.0) as u8 + 97) as char)
            .collect()
    }

    #[test]
    fn should_sha3_a_file() {
        // temp file
        let mut path = env::temp_dir();
        path.push(random_str(8));
        // Prepare file
        {
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(b"something").unwrap();
        }

        let mut file = BufReader::new(fs::File::open(&path).unwrap());
        // when
        let hash = sha3(&mut file).unwrap();

        // then
        assert_eq!(format!("{:?}", hash),
                   "68371d7e884c168ae2022c82bd837d51837718a7f7dfb7aa3f753074a35e1d87");
    }
}
