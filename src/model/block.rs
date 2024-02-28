use log::info;
use serde::{ Deserialize, Serialize };
use chrono::prelude::*;

use super::hash::Hash;

const DIFFICULTY: &'static str = "0000";

pub type BlockId = u64;
pub type Nonce = u64;
pub type Timestamp = i64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub id: BlockId,
    pub timestamp: Timestamp,
    pub nonce: Nonce,
    pub hash: Hash,
    pub previous_hash: Hash,
}

impl Header {
    fn new(
        id: BlockId,
        timestamp: Timestamp,
        nonce: Nonce,
        hash: Hash,
        previous_hash: Hash
    ) -> Self {
        Header {
            id,
            timestamp,
            nonce,
            hash,
            previous_hash,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: Header,
    pub data: String,
}

impl Block {
    pub fn new(id: BlockId, previous_hash: &Hash, data: String) -> Self {
        let now = Utc::now().timestamp();

        let (nonce, hash) = Block::mine_block(id, now, &previous_hash, &data);

        Self {
            header: Header::new(id, now, nonce, hash, previous_hash.clone()),
            data,
        }
    }

    pub fn genesis() -> Self {
        Self {
            header: Header::new(
                0,
                Utc::now().timestamp(),
                0,
                Hash::wrap(
                    String::from("0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43")
                ),
                Hash::wrap(String::from("GENESIS!"))
            ),
            data: String::from("GENESIS!"),
        }
    }

    pub fn is_genesis(&self) -> bool {
        self.header.id == 0
    }

    pub fn is_valid(&self, previous_block: &Block) -> bool {
        if self.header.previous_hash != previous_block.header.hash {
            return false;
        }

        match self.header.hash.decode() {
            Ok(_) => (),
            Err(_) => {
                return false;
            }
        }

        if !self.header.hash.unwrap().starts_with(DIFFICULTY) {
            return false;
        }

        if self.header.id != previous_block.header.id + 1 {
            return false;
        }

        if self.regenerate_hash() != self.header.hash {
            return false;
        }

        true
    }

    pub fn regenerate_hash(&self) -> Hash {
        let data =
            serde_json::json!({
            "id": self.header.id,
            "timestamp": self.header.timestamp,
            "nonce": self.header.nonce,
            "previous_hash": self.header.previous_hash,
            "data": self.data,
        });

        Hash::new(&data.to_string())
    }

    pub fn calculate_hash(
        id: BlockId,
        timestamp: Timestamp,
        previous_hash: &Hash,
        data: &str,
        nonce: Nonce
    ) -> Hash {
        let data =
            serde_json::json!({
            "id": id,
            "timestamp": timestamp,
            "previous_hash": previous_hash,
            "data": data,
            "nonce": nonce,
        });

        Hash::new(&data.to_string())
    }

    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    fn mine_block(
        id: BlockId,
        timestamp: Timestamp,
        previous_hash: &Hash,
        data: &str
    ) -> (Nonce, Hash) {
        info!("mining block...");
        let mut nonce = 0;

        loop {
            if nonce % 100000 == 0 {
                info!("nonce: {}", nonce);
            }
            let hash = Block::calculate_hash(id, timestamp, &previous_hash, data, nonce);

            if hash.matches_difficulty(DIFFICULTY) {
                info!("mined! nonce: {}, hash: {}", nonce, hash);
                return (nonce, hash);
            }

            nonce += 1;
        }
    }
}
