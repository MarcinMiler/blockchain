use super::block::Block;

#[derive(Clone)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
}

impl Blockchain {
    pub fn new() -> Self {
        Self { blocks: vec![] }
    }

    pub fn genesis(&mut self) -> Self {
        Blockchain {
            blocks: vec![Block::genesis()],
        }
    }

    pub fn add_block(&mut self, block: Block) -> Result<bool, String> {
        let latest_block = self.blocks.last().unwrap();

        if !block.is_valid(&latest_block) {
            return Err(String::from("Invalid block!"));
        }

        self.blocks.push(block);
        Ok(true)
    }

    pub fn is_chain_valid(&self) -> bool {
        self.blocks
            .iter()
            .enumerate()
            .all(|(i, block)| {
                if block.is_genesis() {
                    return true;
                }

                self.blocks[i - 1].is_valid(block)
            })
    }

    pub fn choose_chain(&self, local: Blockchain, remote: Blockchain) -> Blockchain {
        let is_local_valid = self.is_chain_valid();
        let is_remote_valid = self.is_chain_valid();

        if !is_local_valid && !is_remote_valid {
            panic!("Both chains are invalid!");
        }

        if is_local_valid && !is_remote_valid {
            return local;
        }

        if !is_local_valid && is_remote_valid {
            return remote;
        }

        if local.blocks.len() > remote.blocks.len() {
            local
        } else {
            remote
        }
    }
}
