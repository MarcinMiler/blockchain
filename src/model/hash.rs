use std::fmt;

use hex::{ self, FromHexError };
use serde::{ Deserialize, Serialize };
use sha2::{ Digest, Sha256 };

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Hash(pub String);

impl Hash {
    pub fn new(data: &str) -> Self {
        let mut hasher = Sha256::new();

        hasher.update(data.as_bytes());
        let pure_hash = hasher.finalize().as_slice().to_owned();

        Self(hex::encode(pure_hash))
    }

    pub fn wrap(hash: String) -> Self {
        Self(hash)
    }

    pub fn matches_difficulty(&self, difficulty: &str) -> bool {
        self.0.starts_with(difficulty)
    }

    pub fn unwrap(&self) -> String {
        self.0.clone()
    }

    pub fn decode(&self) -> Result<Vec<u8>, FromHexError> {
        hex::decode(&self.0)
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash: {}", self.0)
    }
}
