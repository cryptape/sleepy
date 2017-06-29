use std::str::FromStr;
use std::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Error, Visitor};
use rustc_serialize::hex::{FromHexError, ToHex};
use sha3_ext::hash::{H160 as Hash160, H256 as Hash256, H512 as Hash512};


macro_rules! impl_hash {
    ($name: ident, $inner: ident) => {
        /// Lenient hash json deserialization for test json files.
        #[derive(Default, PartialEq, Eq, Hash, PartialOrd, Ord, Clone)]
        pub struct $name(pub $inner);

        impl FromStr for $name {
            type Err = FromHexError;

            fn from_str(s: &str) -> Result<$name, FromHexError> {
                Ok($name($inner::from_str(s)?))
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> $name {
                $name($inner::from(value))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", self.0.to_hex())
            }
        }

        impl From<$name> for $inner {
            fn from(other: $name) -> $inner {
                other.0
            }
        }

        impl From<$inner> for $name {
            fn from(i: $inner) -> Self {
                $name(i)
            }
        }

        impl Copy for $name {}

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
                struct HashVisitor;

                impl<'de> Visitor<'de> for HashVisitor {
                    type Value = $name;
                    
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("struct Hash")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> where E: Error {
                        let value = match value.len() {
                            0 => $inner::from(0),
                            2 if value == "0x" => $inner::from(0),
                            _ if value.starts_with("0x") => $inner::from_str(&value[2..]).map_err(|_| {
                                Error::custom(format!("Invalid hex value {}.", value).as_str())
                            })?,
                            _ => $inner::from_str(value).map_err(|_| {
                                Error::custom(format!("Invalid hex value {}.", value).as_str())
                            })?,
                        };

                        Ok($name(value))
                    }

                    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> where E: Error {
                        self.visit_str(value.as_ref())
                    }
                }

                deserializer.deserialize_str(HashVisitor)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
                let mut hex = "0x".to_owned();
                hex.push_str(&self.0.to_hex());
                serializer.serialize_str(&hex)
            }
        }
    }
}

impl_hash!(H160, Hash160);
impl_hash!(H256, Hash256);
impl_hash!(H512, Hash512);

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use serde_json;
    use sha3_ext::hash;
    use super::H256;

    #[test]
    fn hash_deserialization() {
        let s = r#"["", "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"]"#;
        let deserialized: Vec<H256> = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized, vec![
                   H256::from(0),
                   H256::from_str("5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae").unwrap()
        ]);
    }

    #[test]
    fn hash_into() {
        assert_eq!(H256::from(0), hash::H256::from(0).into());
    }
}