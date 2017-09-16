use std::fmt;
use std::default::Default;
use std::ops::{Add, Sub, Mul, Neg};
use std::marker::PhantomData;
use num::{BigUint, Zero, One};
use num::bigint::ToBigUint;

use fields::{Field, FieldElem, P192, R192, P256, R256, P521, R521};

#[allow(non_snake_case)]
// Weierstrass curve with large characteristic field.
pub trait Curve<F: Field, G: Field>: Clone + Default { // To do:  Implement binary curves.
    fn AN3(&self) -> bool; // If A = -3
    fn unserialize(&self, s: &Vec<usize>) -> AffinePoint<Self, F, G>;

    // Standard curve paraemters:
    fn A(&self) -> FieldElem<F>;
    fn B(&self) -> FieldElem<F>;
    fn G(&self) -> AffinePoint<Self, F, G>;
}

#[derive(Clone, Debug)]
pub struct AffinePoint<C: Curve<F, G>, F: Field, G: Field> {
    pub x: FieldElem<F>,
    pub y: FieldElem<F>,
    pub c: PhantomData<C>,
    pub f: PhantomData<F>,
    pub g: PhantomData<G>,
}

#[derive(Clone, Debug)]
pub struct JacobianPoint<C: Curve<F, G>, F: Field, G: Field> {
    pub x: FieldElem<F>,
    pub y: FieldElem<F>,
    pub z: FieldElem<F>,
    pub c: PhantomData<C>,
    pub f: PhantomData<F>,
    pub g: PhantomData<G>,
    
}

// Constant-time exponentiation/scalar-multiplication.
fn pow<T: Zero + Add<T> + Clone>(this: &T, cand_exp: &BigUint, order: &BigUint) -> T {
    // Montgomery Ladder.
    let zer: BigUint = Zero::zero();
    let one: BigUint = One::one();
    let mut exp: BigUint = cand_exp + order;

    if exp.bits() == order.bits() { exp = &exp + order; }

    let m = exp.bits() + 1;
    let mut r0: T = Zero::zero();
    let mut r1: T = (*this).clone();

    for i in 0..m {
        if ((&one << (m - i - 1)) & &exp) == zer {
            r1 = r0.clone() + r1.clone();
            r0 = r0.clone() + r0.clone();
        } else {
            r0 = r0.clone() + r1.clone();
            r1 = r1.clone() + r1.clone();
        }
    }

    r0
}

impl<C: Curve<F, G>, F: Field, G: Field> PartialEq for AffinePoint<C, F, G> {
    fn eq(&self, other: &AffinePoint<C, F, G>) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> PartialEq for JacobianPoint<C, F, G> {
    fn eq(&self, other: &JacobianPoint<C, F, G>) -> bool {
        self.to_affine() == other.to_affine()
    }
}

// Notes:  Ordering is omitted because elliptic curve groups have no norm.

impl<C: Curve<F, G>, F: Field, G: Field> Add<AffinePoint<C, F, G>> for AffinePoint<C, F, G> {
    type Output = AffinePoint<C, F, G>;
    fn add(self, other: AffinePoint<C, F, G>) -> AffinePoint<C, F, G> {
        if self.is_zero() {
            other.clone()
        } else if other.is_zero() {
            self.clone()
        } else if self.x != other.x {
            let m = (other.y.clone() - self.y.clone()) / (other.x.clone() - self.x.clone());

            let x3 = (m.clone() * m.clone()) - self.x.clone() - other.x.clone();
            let y3 = m * (self.x.clone() - x3.clone()) - self.y.clone();

            AffinePoint { x: x3, y: y3, c: PhantomData, f: PhantomData, g: PhantomData }
        } else if self.y != other.y || self.y.limbs.bits() == 0 {
            Zero::zero()
        } else {
            let c: C = Default::default();
            let two: FieldElem<F> = FieldElem {
                limbs: 2isize.to_biguint().unwrap(),
                f: PhantomData
            };
            let three: FieldElem<F> = FieldElem {
                limbs: 3isize.to_biguint().unwrap(),
                f: PhantomData
            };

            let m = ((three * (self.x.clone() * self.x.clone())) + c.A()) / (two.clone() * self.y.clone());

            let x3 = (m.clone() * m.clone()) - (self.x.clone() * two.clone());
            let y3 = m.clone() * (self.x.clone() - x3.clone()) - self.y.clone();

            AffinePoint { x: x3, y: y3, c: PhantomData, f: PhantomData, g: PhantomData }
        }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> Add<JacobianPoint<C, F, G>> for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn add(self, other: JacobianPoint<C, F, G>) -> JacobianPoint<C, F, G> {
        if self.is_zero() {
            other.clone()
        } else if other.is_zero() {
            self.clone()
        } else if self.x == other.x && self.y == other.y && self.z == other.z {
            self.double()
        } else {
            let z12 = self.z.clone() * self.z.clone();
            let z13 = z12.clone() * self.z.clone();
            let z22 = other.z.clone() * other.z.clone();
            let z23 = z22.clone() * other.z.clone();

            let u1 = self.x.clone() * z22.clone();
            let u2 = other.x.clone() * z12.clone();
            let s1 = self.y.clone() * z23.clone();
            let s2 = other.y.clone() * z13.clone();

            if u1 == u2 {
                if s1 != s2 { // P1 = +/- P2
                    Zero::zero()
                } else {
                    self.double()
                }
            } else {
                let h = u2.clone() - u1.clone();
                let h2 = h.clone() * h.clone();
                let h3 = h2.clone() * h.clone();

                let r = s2.clone() - s1.clone();
                let u1h2 = u1.clone() * h2.clone();

                let x3 = (r.clone() * r.clone()) - h3.clone() - u1h2.clone() - u1h2.clone();
                let y3 = r.clone() * (u1h2.clone() - x3.clone()) - (s1.clone() * h3.clone());
                let z3 = h.clone() * self.z.clone() * other.z.clone();

                JacobianPoint { x: x3, y: y3, z: z3, c: PhantomData, f: PhantomData, g: PhantomData }
            }
        }
    }
}

fn add_jacobian_to_affine<C: Curve<F, G>, F: Field, G: Field>(this: &JacobianPoint<C, F, G>, other: &AffinePoint<C, F, G>) -> JacobianPoint<C, F, G> {
    if this.is_zero() {
        (*other).to_jacobian()
    } else if other.is_zero() {
        (*this).clone()
    } else {
        let z2 = this.z.clone() * this.z.clone();
        let c = (other.x.clone() * z2.clone()) - this.x.clone();

        if c.is_zero() {
            if (other.y.clone() * z2.clone() * this.z.clone()) == this.y { // Same point
                this.double()
            } else { // Negatives
                Zero::zero()
            }
        } else {
            let d = (other.y.clone() * z2.clone() * this.z.clone()) - this.y.clone();
            let c2 = c.clone() * c.clone();

            let x3 = (d.clone() * d.clone()) - (c2.clone() * c.clone()) - (c2.clone() * (this.x.clone() + this.x.clone()));
            let y3 = d.clone() * ((this.x.clone() * c2.clone()) - x3.clone()) - (this.y.clone() * c2.clone() * c.clone());
            let z3 = this.z.clone() * c.clone();

            JacobianPoint { x: x3, y: y3, z: z3, c: PhantomData, f: PhantomData, g: PhantomData }
        }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> Add<JacobianPoint<C, F, G>> for AffinePoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn add(self, other:JacobianPoint<C, F, G>) -> JacobianPoint<C, F, G> { add_jacobian_to_affine(&other, &self) }
}

impl<C: Curve<F, G>, F: Field, G: Field> Add<AffinePoint<C, F, G>> for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn add(self, other:AffinePoint<C, F, G>) -> JacobianPoint<C, F, G> { add_jacobian_to_affine(&self, &other) }
}

impl<C: Curve<F, G>, F: Field, G: Field> Sub<AffinePoint<C, F, G>> for AffinePoint<C, F, G> {
    type Output = AffinePoint<C, F, G>;
    fn sub(self, other: AffinePoint<C, F, G>) -> AffinePoint<C, F, G> { self + (-other) }
}

impl<C: Curve<F, G>, F: Field, G: Field> Sub<JacobianPoint<C, F, G>> for AffinePoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn sub(self, other: JacobianPoint<C, F, G>) -> JacobianPoint<C, F, G> { self + (-other) }
}

impl<C: Curve<F, G>, F: Field, G: Field> Sub<JacobianPoint<C, F, G>> for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn sub(self, other: JacobianPoint<C, F, G>) -> JacobianPoint<C, F, G> { self + (-other) }
}

impl<C: Curve<F, G>, F: Field, G: Field> Sub<AffinePoint<C, F, G>> for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn sub(self, other: AffinePoint<C, F, G>) -> JacobianPoint<C, F, G> { self + (-other) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<FieldElem<G>> for AffinePoint<C, F, G> {
    type Output = AffinePoint<C, F, G>;
    fn mul(self, other: FieldElem<G>) -> AffinePoint<C, F, G> { self.pow(&other.limbs) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<AffinePoint<C, F, G>> for FieldElem<G> {
    type Output = AffinePoint<C, F, G>;
    fn mul(self, other: AffinePoint<C, F, G>) -> AffinePoint<C, F, G> { other.pow(&self.limbs) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<FieldElem<G>> for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn mul(self, other: FieldElem<G>) -> JacobianPoint<C, F, G> { self.pow(&other.limbs) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<JacobianPoint<C, F, G>> for FieldElem<G> {
    type Output = JacobianPoint<C, F, G>;
    fn mul(self, other: JacobianPoint<C, F, G>) -> JacobianPoint<C, F, G> { other.pow(&self.limbs) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<BigUint> for AffinePoint<C, F, G> {
    type Output = AffinePoint<C, F, G>;
    fn mul(self, other: BigUint) -> AffinePoint<C, F, G> { self.pow(&other) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<AffinePoint<C, F, G>> for BigUint {
    type Output = AffinePoint<C, F, G>;
    fn mul(self, other: AffinePoint<C, F, G>) -> AffinePoint<C, F, G> { other.pow(&self) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<BigUint> for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn mul(self, other: BigUint) -> JacobianPoint<C, F, G> { self.pow(&other) }
}

impl<C: Curve<F, G>, F:Field, G: Field> Mul<JacobianPoint<C, F, G>> for BigUint {
    type Output = JacobianPoint<C, F, G>;
    fn mul(self, other: JacobianPoint<C, F, G>) -> JacobianPoint<C, F, G> { other.pow(&self) }
}

impl<C: Curve<F, G>, F: Field, G: Field> Neg for AffinePoint<C, F, G> {
    type Output = AffinePoint<C, F, G>;
    fn neg(self) -> AffinePoint<C, F, G> {
        AffinePoint { x: self.x.clone(), y: -(self.y.clone()), c: PhantomData, f: PhantomData, g: PhantomData }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> Neg for JacobianPoint<C, F, G> {
    type Output = JacobianPoint<C, F, G>;
    fn neg(self) -> JacobianPoint<C, F, G> {
        JacobianPoint { x: self.x.clone(), y: -(self.y.clone()), z: self.z.clone(), c: PhantomData, f: PhantomData, g: PhantomData }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> Zero for AffinePoint<C, F, G> {
    fn zero() -> AffinePoint<C, F, G> {
        AffinePoint { x: Zero::zero(), y: Zero::zero(), c: PhantomData, f: PhantomData, g: PhantomData }
    }

    fn is_zero(&self) -> bool { self.x.is_zero() && self.y.is_zero() }
}

impl<C: Curve<F, G>, F: Field, G: Field> Zero for JacobianPoint<C, F, G> {
    fn zero() -> JacobianPoint<C, F, G> {
        JacobianPoint { x: One::one(), y: One::one(), z: Zero::zero(), c: PhantomData, f: PhantomData, g: PhantomData }
    }

    fn is_zero(&self) -> bool { self.z.is_zero() }
}

impl<C: Curve<F, G>, F: Field, G: Field> AffinePoint<C, F, G> {
    pub fn is_valid(&self) -> bool {
        if self.is_zero() {
            true
        } else {
            let c: C = Default::default();
            let y2 = self.y.clone() * self.y.clone();
            let x = (self.x.clone() * self.x.clone() * self.x.clone()) + (c.A() * self.x.clone()) + c.B();

            y2 == x
        }
    }

    pub fn serialize(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self.x.serialize();

        if self.y.limbs < (-self.y.clone()).limbs { out.push(0) }
        else { out.push(1) }

        out
    }

    fn pow(&self, exp: &BigUint) -> AffinePoint<C, F, G> {
        let g: G = Default::default();
        pow(self, exp, &g.modulus())
    }

    pub fn to_jacobian(&self) -> JacobianPoint<C, F, G> {
        let p = JacobianPoint {
            x: self.x.clone(),
            y: self.y.clone(),
            z: One::one(),
            c: PhantomData, f: PhantomData, g: PhantomData,
        };

        if self.is_zero() {
            Zero::zero()
        } else {
            p
        }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> JacobianPoint<C, F, G> {
    pub fn is_valid(&self) -> bool {
        if self.is_zero() {
            true
        } else {
            let c: C = Default::default();
            let z4 = self.z.clone() * self.z.clone() * self.z.clone() * self.z.clone();

            let y2 = self.y.clone() * self.y.clone();
            let x = (self.x.clone() * self.x.clone() * self.x.clone()) + (c.A() * self.x.clone() * z4.clone())
                + (c.B() * z4.clone() * self.z.clone() * self.x.clone());

            y2 == x
        }
    }

    fn pow(&self, exp: &BigUint) -> JacobianPoint<C, F, G> { // Replace with generic
        let g: G = Default::default();
        pow(self, exp, &g.modulus())
    }

    pub fn double(&self) -> JacobianPoint<C, F, G> {
        let c: C = Default::default();
        let f: F = Default::default();

        let y2 = self.y.clone() * self.y.clone();
        let y4 = y2.clone() * y2.clone();

        let z2 = self.z.clone() * self.z.clone();

        let xy2 = self.x.clone() * y2.clone();
        let yz = self.y.clone() * self.z.clone();

        let v = xy2.clone() + xy2.clone() + xy2.clone() + xy2.clone();
        let w: FieldElem<F>;

        if c.AN3() {
            let tmp = (self.x.clone() + z2.clone()) * (self.x.clone() - z2.clone());
            w = tmp.clone() + tmp.clone() + tmp.clone();
        } else {
            let x2 = self.x.clone() * self.x.clone();
            let z4  = z2.clone() * z2.clone();
            w = x2.clone() + x2.clone() + x2.clone() + (c.A() * z4.clone());
        }

        let v2 = f.reduce(&v.limbs << 1);
        let y84 = f.reduce(&y4.limbs << 3);

        let xf = (w.clone() * w.clone()) - v2;
        let yf = (w.clone() * (v.clone() - xf.clone())) - y84;
        let zf = yz.clone() + yz.clone();

        JacobianPoint { x: xf, y: yf, z: zf, c: PhantomData, f: PhantomData, g: PhantomData }
    }

    pub fn to_affine(&self) -> AffinePoint<C, F, G> {
        if self.is_zero() {
            Zero::zero()
        } else {
            let zinv = self.z.invert();

            AffinePoint {
                x: self.x.clone() * zinv.clone() * zinv.clone(),
                y: self.y.clone() * zinv.clone() * zinv.clone() * zinv.clone(),
                c: PhantomData, f: PhantomData, g: PhantomData,
            }
        }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> fmt::Display for AffinePoint<C, F, G> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        } else {
            return write!(f, "({}, {})", self.x.limbs, self.y.limbs)
        }
    }
}

impl<C: Curve<F, G>, F: Field, G: Field> fmt::Display for JacobianPoint<C, F, G> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        } else {
            return write!(f, "({} : {} : {})", self.x.limbs, self.y.limbs, self.z.limbs)
        }
    }
}

fn unserialize<C: Curve<F, G>, F: Field, G: Field>(c: &C, s: &Vec<usize>) -> (FieldElem<F>, FieldElem<F>) {
    let f: F = Default::default();

    let mut t = s.clone();
    let sign = t.pop().unwrap();

    let x = f.unserialize(&t);
    let (y1, y2) = ((x.clone() * x.clone() * x.clone()) + (c.A() * x.clone()) + c.B()).sqrt();

    if (sign == 0) ^ (y1.limbs < (y2).limbs) {
        (x, y2)
    } else {
        (x, y1)
    }
}

#[derive(Clone, Default, Debug)]
pub struct C192;
impl Curve<P192, R192> for C192 {
    fn AN3(&self) -> bool { true }
    fn A(&self) -> FieldElem<P192> {
        -(FieldElem { limbs: 3isize.to_biguint().unwrap(), f: PhantomData })
    }

    fn B(&self) -> FieldElem<P192> {
        FieldElem { 
            limbs: BigUint::parse_bytes(b"64210519e59c80e70fa7e9ab72243049feb8deecc146b9b1", 16).unwrap(),
            f: PhantomData
        }
    }

    fn G(&self) -> AffinePoint<C192, P192, R192> {
        AffinePoint {
            x: FieldElem { // To do:  Implement FromStrRadix for FieldElem
                limbs: BigUint::parse_bytes(b"188da80eb03090f67cbf20eb43a18800f4ff0afd82ff1012", 16).unwrap(),
                f: PhantomData
            },
            y: FieldElem {
                limbs: BigUint::parse_bytes(b"07192b95ffc8da78631011ed6b24cdd573f977a11e794811", 16).unwrap(),
                f: PhantomData
            },
            c: PhantomData, f: PhantomData, g: PhantomData,
        }
    }

    fn unserialize(&self, s: &Vec<usize>) -> AffinePoint<C192, P192, R192> {
        let (x, y) = unserialize(self, s);

        AffinePoint { x: x, y: y, c: PhantomData, f: PhantomData, g: PhantomData }
    }
}

#[derive(Clone, Default, Debug)]
pub struct C256;
impl Curve<P256, R256> for C256 {
    fn AN3(&self) -> bool { true }
    fn A(&self) -> FieldElem<P256> {
        -(FieldElem { limbs: 3isize.to_biguint().unwrap(), f: PhantomData })
    }

    fn B(&self) -> FieldElem<P256> {
        FieldElem {
            limbs: BigUint::parse_bytes(b"5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b", 16).unwrap(),
            f: PhantomData,
        }
    }

    fn G(&self) -> AffinePoint<C256, P256, R256> {
        AffinePoint {
            x: FieldElem { // To do:  Implement FromStrRadix for FieldElem
                limbs: BigUint::parse_bytes(b"6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296", 16).unwrap(),
                f: PhantomData
            },
            y: FieldElem {
                limbs: BigUint::parse_bytes(b"4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5", 16).unwrap(),
                f: PhantomData
            },
            c: PhantomData, f: PhantomData, g: PhantomData
        }
    }

    fn unserialize(&self, s: &Vec<usize>) -> AffinePoint<C256, P256, R256> {
        let (x, y) = unserialize(self, s);

        AffinePoint { x: x, y: y, c: PhantomData, f: PhantomData, g: PhantomData }
    }}

#[derive(Clone, Default, Debug)]
pub struct C521;
impl Curve<P521, R521> for C521 {
    fn AN3(&self) -> bool { true }
    fn A(&self) -> FieldElem<P521> {
        -(FieldElem {
            limbs: 3isize.to_biguint().unwrap(),
            f: PhantomData
        })
    }

    fn B(&self) -> FieldElem<P521> {
        FieldElem {
            limbs: BigUint::parse_bytes(b"051953eb9618e1c9a1f929a21a0b68540eea2da725b99b315f3b8b489918ef109e156193951ec7e937b1652c0bd3bb1bf073573df883d2c34f1ef451fd46b503f00", 16).unwrap(),
            f: PhantomData
        }
    }

    fn G(&self) -> AffinePoint<C521, P521, R521> {
        AffinePoint {
            x: FieldElem { // To do:  Implement FromStrRadix for FieldElem
                limbs: BigUint::parse_bytes(b"c6858e06b70404e9cd9e3ecb662395b4429c648139053fb521f828af606b4d3dbaa14b5e77efe75928fe1dc127a2ffa8de3348b3c1856a429bf97e7e31c2e5bd66", 16).unwrap(),
                f: PhantomData
            },
            y: FieldElem {
                limbs: BigUint::parse_bytes(b"11839296a789a3bc0045c8a5fb42c7d1bd998f54449579b446817afbd17273e662c97ee72995ef42640c550b9013fad0761353c7086a272c24088be94769fd16650", 16).unwrap(),
                f: PhantomData
            },
            c: PhantomData, f: PhantomData, g: PhantomData
        }
    }

    fn unserialize(&self, s: &Vec<usize>) -> AffinePoint<C521, P521, R521> {
        let (x, y) = unserialize(self, s);

        AffinePoint { x: x, y: y, c: PhantomData, f: PhantomData, g: PhantomData }
    }}

// -------------------------------------------------------------------------
// Unit Tests
// -------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use test::Bencher;    
    use std::marker::PhantomData;
    use num::{BigUint, One};
    use num::bigint::ToBigUint;

    use fields::{FieldElem, P192, R192};
    use super::{AffinePoint, Curve, C192, C256, C521};

    #[test]
    fn accept_valid_point() {
        let c: C192 = C192;
        assert_eq!(c.G().is_valid(), true)
    }

    #[test]
    fn reject_invalid_point() {
        let c: C192 = C192;
        let one: BigUint = One::one();
        let p: AffinePoint<C192, P192, R192> = AffinePoint {
            x: c.G().x,
            y: FieldElem { limbs: c.G().y.limbs + one, f: PhantomData },
            c: PhantomData, f: PhantomData, g: PhantomData,
        };

        assert_eq!(p.is_valid(), false)
    }

    #[test]
    fn base_point_field_is_r192() {
        let c: C192 = C192;
        let one: FieldElem<R192> = FieldElem { limbs: One::one(), f: PhantomData };

        let x: FieldElem<R192> = FieldElem { limbs: 3isize.to_biguint().unwrap(), f: PhantomData };

        let y = x.invert();

        let a = x.clone() * c.G();
        let b = y.clone() * a.clone();

        assert!(x.clone() != y.clone());
        assert!((x.clone() * y.clone()) == one);
        assert!(c.G() != a);
        assert!(c.G() == b);
    }

    #[test]
    fn affine_point_multiplication() {
        let sec: BigUint = BigUint::parse_bytes(b"7e48c5ab7f43e4d9c17bd9712627dcc76d4df2099af7c8e5", 16).unwrap();
        let x: BigUint = BigUint::parse_bytes(b"48162eae1116dbbd5b7a0d9494ff0c9b414a31ce3d8b273f", 16).unwrap();
        let y: BigUint = BigUint::parse_bytes(b"4c221e09f96b3229c95af490487612c8e3bd81704724eeda", 16).unwrap();

        let c: C192 = C192;

        let a = sec * c.G();

        assert_eq!(a.x.limbs, x);
        assert_eq!(a.y.limbs, y);
    }

    #[test]
    fn jacobian_point_multiplication_c192() {
        let sec: BigUint = BigUint::parse_bytes(b"7e48c5ab7f43e4d9c17bd9712627dcc76d4df2099af7c8e5", 16).unwrap();
        let x: BigUint = BigUint::parse_bytes(b"48162eae1116dbbd5b7a0d9494ff0c9b414a31ce3d8b273f", 16).unwrap();
        let y: BigUint = BigUint::parse_bytes(b"4c221e09f96b3229c95af490487612c8e3bd81704724eeda", 16).unwrap();

        let c: C192 = C192;

        let a = (sec * c.G().to_jacobian()).to_affine();

        assert_eq!(a.x.limbs, x);
        assert_eq!(a.y.limbs, y);
    }

    #[test]
    fn mixed_point_addition_c192() {
        let c: C192 = C192;

        let a = c.G().to_jacobian().double() + c.G();
        let b = c.G() + c.G().to_jacobian().double();
        let c = c.G().to_jacobian().double() + c.G().to_jacobian();

        assert_eq!(a, c);
        assert_eq!(b, c);
    }

    #[test]
    fn jacobian_point_multiplication_c256() {
        let sec: BigUint = BigUint::parse_bytes(b"26254a72f6a0ce35958ce62ff0cea754b84ac449b2b340383faef50606d03b01", 16).unwrap();
        let x: BigUint = BigUint::parse_bytes(b"6ee12372b80bad6f5432d0e6a3f02199db2b1617414f7fd8fe90b6bcf8b7aa68", 16).unwrap();
        let y: BigUint = BigUint::parse_bytes(b"5c71967784765fe888995bfd9a1fb76f329630018430e1d9aca7b59dc672cad8", 16).unwrap();

        let c: C256 = C256;

        let a = (sec.clone() * c.G().to_jacobian()).to_affine();

        assert_eq!(a.x.limbs, x);
        assert_eq!(a.y.limbs, y);
    }

    #[bench]
    fn bench_point_mult_c192(b: &mut Bencher) {
        let c: C192 = C192;
        let sec: BigUint = BigUint::parse_bytes(b"7e48c5ab7f43e4d9c17bd9712627dcc76d4df2099af7c8e5", 16).unwrap();
        let p = c.G().to_jacobian();

        b.iter(|| { sec.clone() * p.clone() })
    }

    #[bench]
    fn bench_point_mult_c192_right_zeroes(b: &mut Bencher) {
        let c: C192 = C192;
        let sec: BigUint = BigUint::parse_bytes(b"7e0000000000000000000000000000000000000000000000", 16).unwrap();
        let p = c.G().to_jacobian();

        b.iter(|| { sec.clone() * p.clone() })
    }

    #[bench]
    fn bench_point_mult_c192_left_zeroes(b: &mut Bencher) {
        let c: C192 = C192;
        let sec: BigUint = 3isize.to_biguint().unwrap();
        let p = c.G().to_jacobian();

        b.iter(|| { sec.clone() * p.clone() })
    }

    #[bench]
    fn bench_point_mult_c521(b: &mut Bencher) {
        let c: C521 = C521;
        let sec: BigUint = BigUint::parse_bytes(b"7e48c5ab7f43e4d9c17bd9712627dcc76d4df2099af7c8e5", 16).unwrap();
        let p = c.G().to_jacobian();

        b.iter(|| { sec.clone() * p.clone() })
    }
}
