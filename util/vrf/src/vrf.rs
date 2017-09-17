use ecc::fields::{Field, P256, R256};
use ecc::curves::{Curve, C256, AffinePoint};
use num::BigUint;
use num::bigint::RandBigInt;
use rand;
use sha3::{Digest, Sha3_256};

pub fn sha3(b: &[u8]) -> Vec<u8> {
    let mut hasher = Sha3_256::default();

    // write input message
    hasher.input(b);

    // read hash digest
    hasher.result().to_vec()

}

#[derive(Clone, Debug)]
pub struct PublicKey {
    point: AffinePoint<C256, P256, R256>,
}

impl PublicKey {
    pub fn new(sk: PrivateKey) -> Self {
        let c = C256::default();
        PublicKey {
            point: sk.x.clone() * c.G()
        }
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let x = BigUint::from_bytes_be(&b[0..256]);
        let y = BigUint::from_bytes_be(&b[256..512]);
        
        PublicKey {
            point: AffinePoint::new(x, y)
        }

    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut x = self.point.x.limbs.to_bytes_be();
        let mut y = self.point.y.limbs.to_bytes_be();
        let xl = x.len();
        let yl = y.len();
        let mut fi = vec![0; 256-xl];
        let mut mid = vec![0; 256-yl];
        fi.append(&mut x);
        fi.append(&mut mid);
        fi.append(&mut y);
        fi
    }
}

#[derive(Default, Clone, Debug)]
pub struct PrivateKey {
    x: BigUint,
}

impl PrivateKey {
    pub fn new() -> Self {
        let mut rng = match rand::OsRng::new() {
            Ok(g) => g,
            Err(e) => panic!("Could not load the OS' RNG! Error: {}", e)
        };

        let f = P256::default();

        PrivateKey {
            x: rng.gen_biguint_below(&f.modulus())
        }
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        PrivateKey {
            x: BigUint::from_bytes_be(b)
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.x.to_bytes_be()
    }

    // pub fn vrf(&self, msg: &[u8]) -> (Vec<u8>, Vec<u8>) {

    // }

    // pub fn hash_to_curve(hx: &[u8]) -> (BigUint, BigUint) {

    // }
}